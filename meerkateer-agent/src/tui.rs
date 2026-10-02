use std::{
    collections::VecDeque,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context, ensure};
use chrono::{DateTime, Utc};
use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    AgentConfig, ControlPlane, RuntimeStatus, SignalKind, SignalSelection, collector, diagnostics,
    enroll_with_token, load_config, load_runtime_status, runtime_status_path,
    save_signal_selection, validate_server_url,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const EVENT_LIMIT: usize = 8;
const SIGNALS: [SignalKind; 4] = [
    SignalKind::Cpu,
    SignalKind::Memory,
    SignalKind::Disk,
    SignalKind::Process,
];

const NAVY: Color = Color::Rgb(7, 25, 82);
const BLUE: Color = Color::Rgb(20, 101, 183);
const MINT: Color = Color::Rgb(52, 211, 153);
const GOLD: Color = Color::Rgb(251, 191, 36);
const CORAL: Color = Color::Rgb(251, 113, 133);
const MUTED: Color = Color::Rgb(148, 163, 184);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Signals,
    Diagnostics,
    Help,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Overview, Self::Signals, Self::Diagnostics, Self::Help];

    const fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Signals => 1,
            Self::Diagnostics => 2,
            Self::Help => 3,
        }
    }

    const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    const fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone)]
struct ConfigView {
    destination: ControlPlane,
    server_url: String,
    display_name: String,
    agent_id: Option<Uuid>,
    project_id: Option<Uuid>,
    credential_expires_at: Option<String>,
    pending_batch: bool,
    next_sequence: u64,
}

impl From<&AgentConfig> for ConfigView {
    fn from(config: &AgentConfig) -> Self {
        Self {
            destination: config.destination,
            server_url: config.server_url.clone(),
            display_name: config.display_name.clone(),
            agent_id: config.agent_id,
            project_id: config.project_id,
            credential_expires_at: config.credential_expires_at.clone(),
            pending_batch: config.pending_batch.is_some(),
            next_sequence: config.next_sequence,
        }
    }
}

struct SetupForm {
    server_url: String,
    display_name: String,
    token: Zeroizing<String>,
    selected: usize,
}

impl SetupForm {
    fn new() -> Self {
        let display_name = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "my-server".to_owned());
        Self {
            server_url: "https://".to_owned(),
            display_name,
            token: Zeroizing::new(String::new()),
            selected: 0,
        }
    }

    fn selected_value_mut(&mut self) -> Option<&mut String> {
        match self.selected {
            1 => Some(&mut self.server_url),
            2 => Some(&mut self.display_name),
            3 => Some(&mut self.token),
            _ => None,
        }
    }
}

struct App {
    config_path: PathBuf,
    config: Option<ConfigView>,
    selection: SignalSelection,
    snapshot: Option<collector::HostSnapshot>,
    setup: Option<SetupForm>,
    tab: Tab,
    selected_signal: usize,
    editing_processes: bool,
    process_editor: String,
    status: String,
    events: VecDeque<String>,
    service_status: String,
    runtime: RuntimeStatus,
    connection_report: Option<diagnostics::ConnectionReport>,
    last_refresh: Instant,
    quit: bool,
}

impl App {
    fn load(config_path: &Path) -> anyhow::Result<Self> {
        let mut app = Self {
            config_path: config_path.to_path_buf(),
            config: None,
            selection: SignalSelection::default(),
            snapshot: None,
            setup: None,
            tab: Tab::Overview,
            selected_signal: 0,
            editing_processes: false,
            process_editor: String::new(),
            status: "Starting local lookout...".to_owned(),
            events: VecDeque::new(),
            service_status: service_status(),
            runtime: RuntimeStatus::default(),
            connection_report: None,
            last_refresh: Instant::now(),
            quit: false,
        };
        match load_config(config_path) {
            Ok(config) if config.agent_id.is_none() => {
                let mut form = SetupForm::new();
                form.server_url = config.server_url;
                form.display_name = config.display_name;
                form.selected = 3;
                app.setup = Some(form);
                app.set_status("Previous enrollment was incomplete; paste a fresh one-time token");
            }
            Ok(config) => {
                app.apply_config(&config);
                app.push_event("Protected controller config loaded");
            }
            Err(error) if is_not_found(&error) => {
                app.setup = Some(SetupForm::new());
                app.set_status("Connect this computer to begin");
            }
            Err(error) => return Err(error),
        }
        match load_runtime_status(&runtime_status_path(config_path)) {
            Ok(runtime) => {
                if runtime.last_error.is_some() {
                    app.push_event("The daemon recorded a delivery error");
                }
                app.runtime = runtime;
            }
            Err(error) => app.status = format!("Runtime status could not be read: {error}"),
        }
        Ok(app)
    }

    fn apply_config(&mut self, config: &AgentConfig) {
        self.config = Some(ConfigView::from(config));
        self.selection = config.signals.clone();
        self.process_editor = config.signals.watched_processes.join(", ");
        self.set_status("Controller ready");
    }

    fn reload_config(&mut self) -> anyhow::Result<()> {
        let config = load_config(&self.config_path)?;
        self.apply_config(&config);
        self.runtime = load_runtime_status(&runtime_status_path(&self.config_path))?;
        Ok(())
    }

    async fn refresh_snapshot(&mut self) {
        let watched = if self.selection.enabled.contains(&SignalKind::Process) {
            self.selection.watched_processes.clone()
        } else {
            Vec::new()
        };
        match collector::collect(&watched).await {
            Ok(snapshot) => {
                self.status = format!("Local signals refreshed at {}", snapshot.collected_at);
                self.snapshot = Some(snapshot);
                self.last_refresh = Instant::now();
            }
            Err(error) => {
                self.status = format!("Local collection failed: {error}");
                self.push_event("Local signal refresh failed");
            }
        }
    }

    async fn test_connection(&mut self, server_url: String) {
        self.set_status("Testing storage, DNS, routing, TLS, and API readiness...");
        let report = diagnostics::test_self_hosted_connection(&server_url, &self.config_path).await;
        let ok = report.is_ok();
        self.status = report.summary();
        self.connection_report = Some(report);
        self.push_event(if ok {
            "Connection diagnostics passed; no telemetry was sent"
        } else {
            "Connection diagnostics found a blocking problem"
        });
    }

    fn save_signals(&mut self) {
        let watched_processes = if self.selection.enabled.contains(&SignalKind::Process) {
            match parse_process_names(&self.process_editor) {
                Ok(values) => values,
                Err(error) => {
                    self.status = format!("Signals not saved: {error}");
                    return;
                }
            }
        } else {
            Vec::new()
        };
        let signals = SIGNALS
            .iter()
            .copied()
            .filter(|signal| self.selection.enabled.contains(signal))
            .collect::<Vec<_>>();
        match SignalSelection::from_cli(&signals, watched_processes)
            .and_then(|selection| save_signal_selection(&self.config_path, selection))
        {
            Ok(selection) => {
                self.selection = selection;
                self.process_editor = self.selection.watched_processes.join(", ");
                self.set_status("Signal allowlist saved; the daemon reads it on its next cycle");
                self.push_event("Signal allowlist updated");
                if let Err(error) = self.reload_config() {
                    self.status = format!("Signals saved, but config reload failed: {error}");
                }
            }
            Err(error) => self.status = format!("Signals not saved: {error}"),
        }
    }

    fn push_event(&mut self, message: &str) {
        if self.events.len() == EVENT_LIMIT {
            self.events.pop_back();
        }
        self.events
            .push_front(format!("{}  {message}", Utc::now().format("%H:%M:%S")));
    }

    fn set_status(&mut self, message: &str) {
        self.status.clear();
        self.status.push_str(message);
    }

    fn toggle_signal(&mut self) {
        let signal = SIGNALS[self.selected_signal];
        if !self.selection.enabled.remove(&signal) {
            self.selection.enabled.insert(signal);
        }
        self.set_status("Unsaved signal changes");
    }

    async fn submit_setup(&mut self) {
        let Some(form) = self.setup.as_ref() else {
            return;
        };
        let server_url = form.server_url.trim().to_owned();
        let display_name = form.display_name.trim().to_owned();
        if form.token.trim().is_empty() {
            if let Some(form) = self.setup.as_mut() {
                form.selected = 3;
            }
            self.set_status("Paste the one-time enrollment token");
            return;
        }
        self.test_connection(server_url.clone()).await;
        if self
            .connection_report
            .as_ref()
            .is_none_or(|report| !report.is_ok())
        {
            self.status = format!(
                "Fix the connection checks before enrollment: {}",
                self.status
            );
            return;
        }
        let token = self
            .setup
            .as_mut()
            .map(|form| std::mem::take(&mut form.token))
            .unwrap_or_default();
        self.set_status("Enrolling this computer...");
        match enroll_with_token(
            &self.config_path,
            ControlPlane::SelfHosted,
            &server_url,
            &display_name,
            token.as_str(),
        )
        .await
        {
            Ok(_) => match self.reload_config() {
                Ok(()) => {
                    self.setup = None;
                    self.set_status("Connected. Review Signals and press S to save.");
                    self.push_event("Computer enrolled successfully");
                    self.refresh_snapshot().await;
                }
                Err(error) => self.status = format!("Enrolled, but config reload failed: {error}"),
            },
            Err(error) => {
                self.status = format!("Enrollment failed: {error}");
                if let Some(current) = self.setup.as_mut() {
                    current.selected = 3;
                }
            }
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.setup.is_some() {
            self.handle_setup_key(key).await;
            return;
        }
        if self.editing_processes {
            self.handle_process_editor_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Tab => self.tab = self.tab.next(),
            KeyCode::BackTab => self.tab = self.tab.previous(),
            KeyCode::Char('1') => self.tab = Tab::Overview,
            KeyCode::Char('2') => self.tab = Tab::Signals,
            KeyCode::Char('3') => self.tab = Tab::Diagnostics,
            KeyCode::Char('4') => self.tab = Tab::Help,
            KeyCode::Char('r') => {
                self.service_status = service_status();
                if let Err(error) = self.reload_config() {
                    self.status = format!("Config refresh failed: {error}");
                }
                self.refresh_snapshot().await;
            }
            KeyCode::Char('t') if self.tab == Tab::Diagnostics => {
                if let Some(server_url) =
                    self.config.as_ref().map(|config| config.server_url.clone())
                {
                    self.test_connection(server_url).await;
                } else {
                    self.set_status("No enrolled API URL is available");
                }
            }
            KeyCode::Char('s') if self.tab == Tab::Signals => self.save_signals(),
            KeyCode::Char('e') if self.tab == Tab::Signals => {
                self.editing_processes = true;
                self.set_status("Editing exact process names; Enter applies locally, S saves");
            }
            KeyCode::Char(' ') if self.tab == Tab::Signals => self.toggle_signal(),
            KeyCode::Up if self.tab == Tab::Signals => {
                self.selected_signal = self.selected_signal.saturating_sub(1);
            }
            KeyCode::Down if self.tab == Tab::Signals => {
                self.selected_signal = (self.selected_signal + 1).min(SIGNALS.len() - 1);
            }
            _ => {}
        }
    }

    async fn handle_setup_key(&mut self, key: KeyEvent) {
        let mut submit = false;
        let mut test = false;
        if let Some(form) = self.setup.as_mut() {
            match key.code {
                KeyCode::Esc => self.quit = true,
                KeyCode::Tab | KeyCode::Down => form.selected = (form.selected + 1) % 6,
                KeyCode::BackTab | KeyCode::Up => form.selected = (form.selected + 5) % 6,
                KeyCode::Enter if form.selected == 4 => test = true,
                KeyCode::Enter if form.selected == 5 => submit = true,
                KeyCode::Enter if form.selected == 0 => {
                    "Local / self-hosted is selected. Meerkateer Cloud is coming soon."
                        .clone_into(&mut self.status);
                }
                KeyCode::Enter => form.selected = (form.selected + 1).min(5),
                KeyCode::Backspace => {
                    if let Some(value) = form.selected_value_mut() {
                        value.pop();
                    }
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    if let Some(value) = form.selected_value_mut() {
                        value.push(character);
                    }
                }
                _ => {}
            }
        }
        if submit {
            self.submit_setup().await;
        }
        if test {
            let server_url = self
                .setup
                .as_ref()
                .map(|form| form.server_url.trim().to_owned())
                .unwrap_or_default();
            self.test_connection(server_url).await;
        }
    }

    fn handle_process_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.process_editor = self.selection.watched_processes.join(", ");
                self.editing_processes = false;
                self.set_status("Process edit cancelled");
            }
            KeyCode::Enter => match parse_process_names(&self.process_editor) {
                Ok(values) => {
                    self.selection.watched_processes = values;
                    if !self.selection.watched_processes.is_empty() {
                        self.selection.enabled.insert(SignalKind::Process);
                    }
                    self.editing_processes = false;
                    self.set_status("Unsaved process allowlist; press S to save");
                }
                Err(error) => self.status = format!("Invalid process list: {error}"),
            },
            KeyCode::Backspace => {
                self.process_editor.pop();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.process_editor.push(character);
            }
            _ => {}
        }
    }
}

pub async fn run(config_path: &Path) -> anyhow::Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "tui requires an interactive terminal"
    );
    let mut app = App::load(config_path)?;
    if app.setup.is_none() {
        app.refresh_snapshot().await;
    }

    enable_raw_mode().context("failed to enable terminal raw mode")?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
        let _ = disable_raw_mode();
        return Err(error).context("failed to initialize terminal screen");
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(value) => value,
        Err(error) => {
            let _ = disable_raw_mode();
            let _ = execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                Show
            );
            return Err(error).context("failed to create terminal UI");
        }
    };

    let result = run_loop(&mut terminal, &mut app).await;
    let raw_result = disable_raw_mode();
    let screen_result = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    );
    let cursor_result = terminal.show_cursor();
    result?;
    raw_result.context("failed to restore terminal mode")?;
    screen_result.context("failed to restore terminal screen")?;
    cursor_result.context("failed to restore terminal cursor")?;
    Ok(())
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.quit {
        terminal.draw(|frame| render(frame, app))?;
        if event::poll(POLL_INTERVAL)?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key).await;
        }
        if app.setup.is_none() && app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh_snapshot().await;
        }
    }
    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < 72 || area.height < 22 {
        frame.render_widget(
            Paragraph::new(
                "Meerkateer needs a terminal at least 72 x 22. Resize the window or press Q.",
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title(" Meerkateer ")),
            area,
        );
        return;
    }
    if let Some(form) = app.setup.as_ref() {
        render_setup(frame, area, form, &app.status);
        return;
    }

    let [header, tabs, content, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(12),
        Constraint::Length(3),
    ])
    .areas(area);

    let connection = if app.config.is_some() {
        "CONNECTED"
    } else {
        "LOCAL"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " (o.o) ",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "MEERKATEER CONTROLLER",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                connection,
                Style::default().fg(MINT).add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .style(Style::default().bg(NAVY))
                .borders(Borders::ALL),
        ),
        header,
    );

    let titles = ["1 Overview", "2 Signals", "3 Diagnostics", "4 Help"]
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.tab.index())
            .highlight_style(Style::default().fg(GOLD).add_modifier(Modifier::BOLD))
            .divider("  ")
            .block(Block::default().borders(Borders::BOTTOM)),
        tabs,
    );

    match app.tab {
        Tab::Overview => render_overview(frame, content, app),
        Tab::Signals => render_signals(frame, content, app),
        Tab::Diagnostics => render_diagnostics(frame, content, app),
        Tab::Help => render_help(frame, content),
    }

    let keys = match app.tab {
        Tab::Signals => "↑↓ choose  Space toggle  E processes  S save  R refresh  Q quit",
        Tab::Diagnostics => "T test API  R refresh  Tab next  Q quit",
        _ => "Tab switch  R refresh  Q quit",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", app.status),
                Style::default().fg(Color::White),
            ),
            Span::styled(format!("  {keys}"), Style::default().fg(MUTED)),
        ]))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().bg(NAVY)),
        ),
        footer,
    );
}

#[allow(clippy::too_many_lines)] // Keeping the two-column dashboard layout together aids review.
fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [identity_area, live_area] =
        Layout::horizontal([Constraint::Percentage(43), Constraint::Percentage(57)])
            .spacing(1)
            .areas(area);
    let [identity, events] =
        Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)])
            .spacing(1)
            .areas(identity_area);

    let config_lines = app.config.as_ref().map_or_else(
        || vec![Line::from("Not enrolled")],
        |config| {
            vec![
                field_line("Computer", &config.display_name),
                field_line("API", &config.server_url),
                field_line("Service", &app.service_status),
                field_line("Agent", &short_uuid(config.agent_id)),
                field_line("Workspace", &short_uuid(config.project_id)),
                field_line("Next sequence", &config.next_sequence.to_string()),
                field_line(
                    "Retry queue",
                    if config.pending_batch {
                        "1 durable batch"
                    } else {
                        "empty"
                    },
                ),
                field_line(
                    "Last success",
                    &compact_timestamp(app.runtime.last_success_at.as_deref()),
                ),
                field_line(
                    "Last delivery",
                    if app.runtime.last_error.is_some() {
                        "issue recorded"
                    } else {
                        "healthy / not attempted"
                    },
                ),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(config_lines)
            .wrap(Wrap { trim: true })
            .block(card(" Machine identity ", BLUE)),
        identity,
    );

    let event_items = if app.events.is_empty() {
        vec![ListItem::new("No local session events yet")]
    } else {
        app.events
            .iter()
            .map(|value| ListItem::new(value.as_str()))
            .collect()
    };
    frame.render_widget(
        List::new(event_items).block(card(" This session ", GOLD)),
        events,
    );

    let [gauges, processes] = Layout::vertical([Constraint::Length(12), Constraint::Min(5)])
        .spacing(1)
        .areas(live_area);
    if let Some(snapshot) = app.snapshot.as_ref() {
        let [cpu, memory, disk] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .areas(gauges.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        }));
        frame.render_widget(card(" Live local signals ", MINT), gauges);
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(MINT))
                .ratio((snapshot.cpu_usage_percent / 100.0).clamp(0.0, 1.0))
                .label(format!("CPU  {:.1}%", snapshot.cpu_usage_percent)),
            cpu,
        );
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(BLUE))
                .ratio(ratio(
                    snapshot.memory_used_bytes,
                    snapshot.memory_total_bytes,
                ))
                .label(format!(
                    "Memory  {} / {}",
                    format_bytes(snapshot.memory_used_bytes),
                    format_bytes(snapshot.memory_total_bytes)
                )),
            memory,
        );
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(GOLD))
                .ratio(ratio(snapshot.disk_used_bytes, snapshot.disk_total_bytes))
                .label(format!(
                    "Disk  {} / {}",
                    format_bytes(snapshot.disk_used_bytes),
                    format_bytes(snapshot.disk_total_bytes)
                )),
            disk,
        );

        let process_items = if snapshot.watched_processes.is_empty() {
            vec![ListItem::new("No process names selected")]
        } else {
            snapshot
                .watched_processes
                .iter()
                .map(|process| {
                    let (icon, color) = if process.running {
                        ("●", MINT)
                    } else {
                        ("●", CORAL)
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{icon} "), Style::default().fg(color)),
                        Span::styled(&process.name, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("   {} instance(s)", process.instances)),
                    ]))
                })
                .collect()
        };
        frame.render_widget(
            List::new(process_items).block(card(" Watched processes ", CORAL)),
            processes,
        );
    } else {
        frame.render_widget(
            Paragraph::new("Collecting a local snapshot...")
                .alignment(Alignment::Center)
                .block(card(" Live local signals ", MINT)),
            live_area,
        );
    }
}

fn render_signals(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [choices, detail] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
            .spacing(1)
            .areas(area);
    let items = SIGNALS
        .iter()
        .enumerate()
        .map(|(index, signal)| {
            let checked = app.selection.enabled.contains(signal);
            let marker = if checked { "[x]" } else { "[ ]" };
            let style = if index == app.selected_signal {
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{marker} {}", signal_label(*signal))).style(style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(card(" Signal allowlist ", MINT)),
        choices,
    );

    let editor_style = if app.editing_processes {
        Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let details = vec![
        Line::from(Span::styled(
            "Heartbeat is always sent so disconnects can be detected.",
            Style::default().fg(MINT),
        )),
        Line::from(""),
        Line::from("Process names (exact, comma-separated, maximum 16):"),
        Line::from(Span::styled(
            if app.process_editor.is_empty() {
                "<none>"
            } else {
                &app.process_editor
            },
            editor_style,
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Never collected",
            Style::default().fg(CORAL).add_modifier(Modifier::BOLD),
        )),
        Line::from("Command arguments, environment values, file names/content,"),
        Line::from("usernames, player/chat content, and arbitrary remote commands."),
        Line::from(""),
        Line::from(Span::styled(
            "Changes are local and atomic. The daemon reads them on its next cycle.",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(details)
            .wrap(Wrap { trim: false })
            .block(card(" What leaves this computer ", BLUE)),
        detail,
    );
}

fn render_diagnostics(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [checks, events] =
        Layout::vertical([Constraint::Percentage(62), Constraint::Percentage(38)])
            .spacing(1)
            .areas(area);
    let mut lines = vec![diagnostic_line(
        app.config.is_some(),
        "Protected configuration is readable",
    )];
    if let Some(config) = app.config.as_ref() {
        lines.push(diagnostic_line(
            config.destination == ControlPlane::SelfHosted,
            "Destination is Local / self-hosted Community (Cloud is not open)",
        ));
        lines.push(diagnostic_line(
            validate_server_url(&config.server_url).is_ok(),
            "API URL is HTTPS or loopback HTTP",
        ));
        lines.push(diagnostic_line(
            credential_is_fresh(config.credential_expires_at.as_deref()),
            "Machine credential has not expired",
        ));
        lines.push(diagnostic_line(
            !config.pending_batch,
            if config.pending_batch {
                "One durable telemetry batch is waiting to retry"
            } else {
                "No telemetry batch is waiting to retry"
            },
        ));
        lines.push(diagnostic_line(
            app.runtime.last_error.is_none(),
            &match (
                app.runtime.last_error_code.as_deref(),
                app.runtime.last_error.as_deref(),
            ) {
                (Some(code), Some(message)) => format!("{code}: {message}"),
                _ => "Daemon has not recorded a delivery error".to_owned(),
            },
        ));
        if let Some(hint) = app.runtime.last_error_hint.as_deref() {
            lines.push(Line::from(vec![
                Span::styled(
                    "  NEXT  ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::raw(hint),
            ]));
        }
    }
    lines.push(diagnostic_line(
        app.snapshot.is_some(),
        "Local CPU, memory, disk, and process collector works",
    ));
    lines.push(Line::from(vec![
        Span::styled(
            "  SERVICE  ",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.service_status),
    ]));
    lines.push(Line::from(""));
    if let Some(report) = app.connection_report.as_ref() {
        lines.push(Line::from(Span::styled(
            "Last connection test",
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        )));
        for check in &report.checks {
            lines.push(connection_check_line(check));
        }
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Press T to test storage, DNS, route/proxy/VPN effects, TLS, and public /ready. No credential or telemetry is sent.",
        Style::default().fg(MUTED),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(card(" Local safety checks ", MINT)),
        checks,
    );

    let event_lines = app
        .events
        .iter()
        .map(|value| Line::from(value.as_str()))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(if event_lines.is_empty() {
            vec![Line::from("No events in this TUI session")]
        } else {
            event_lines
        })
        .block(card(" Session events (no secrets) ", GOLD)),
        events,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let text = vec![
        Line::from(Span::styled(
            "The Controller daemon stays headless. This TUI is an on-demand local window.",
            Style::default().fg(MINT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("Tab / Shift+Tab", "move between pages"),
        help_line("1 2 3 4", "jump directly to a page"),
        help_line("R", "reload protected config and refresh local signals"),
        help_line("Space", "toggle the selected signal on the Signals page"),
        help_line("E", "edit exact process names"),
        help_line("S", "atomically save the signal allowlist"),
        help_line(
            "T",
            "test the API readiness endpoint without sending telemetry",
        ),
        help_line(
            "Q / Ctrl+C",
            "close the TUI; the background daemon keeps running",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Automation remains available through inspect, doctor, configure, and run.",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(card(" Keyboard and operating model ", BLUE)),
        area,
    );
}

fn render_setup(frame: &mut Frame<'_>, area: Rect, form: &SetupForm, status: &str) {
    let outer = centered_rect(78, 86, area);
    frame.render_widget(Block::default().style(Style::default().bg(NAVY)), area);
    let [title, intro, fields, privacy, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Min(13),
        Constraint::Length(5),
        Constraint::Length(3),
    ])
    .areas(outer);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                " (o.o)  Connect this computer",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Meerkateer Controller setup",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .alignment(Alignment::Center),
        title,
    );
    frame.render_widget(
        Paragraph::new("Choose where data goes, test the route, then enroll. Cloud is visible but disabled until the hosted service opens.")
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        intro,
    );

    let values = [
        (
            "Destination",
            "Local / self-hosted Community  [Cloud: coming soon]".to_owned(),
        ),
        ("API URL", form.server_url.clone()),
        ("Computer name", form.display_name.clone()),
        ("One-time token", mask_secret(form.token.len())),
        ("", "[ Test connection ]".to_owned()),
        ("", "[ Connect safely ]".to_owned()),
    ];
    let items = values
        .into_iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let marker = if index == form.selected { ">" } else { " " };
            let line = if label.is_empty() {
                format!("{marker} {value}")
            } else {
                format!("{marker} {label}:  {value}")
            };
            let style = if index == form.selected {
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(line).style(style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items).block(card(" Enrollment ", MINT)), fields);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Outbound HTTPS only. No inbound listener or remote shell."),
            Line::from(
                "Test checks disk/config, DNS, proxy/VPN-sensitive routing, TLS, and /ready.",
            ),
            Line::from("Tab moves • Enter selects/tests/connects • Esc exits"),
            Line::from(Span::styled(status, Style::default().fg(MINT))),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        privacy,
    );
    frame.render_widget(
        Paragraph::new("Apache-2.0 Community • local credential stays on this computer")
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED)),
        footer,
    );
}

fn card(title: &'static str, color: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
}

fn field_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<14}"), Style::default().fg(MUTED)),
        Span::raw(value.to_owned()),
    ])
}

fn diagnostic_line(ok: bool, message: &str) -> Line<'static> {
    let (marker, color) = if ok { ("PASS", MINT) } else { ("WARN", GOLD) };
    Line::from(vec![
        Span::styled(
            format!("  {marker:<4}  "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(message.to_owned()),
    ])
}

fn connection_check_line(check: &diagnostics::DiagnosticCheck) -> Line<'_> {
    let (marker, color) = match check.state {
        diagnostics::CheckState::Pass => ("PASS", MINT),
        diagnostics::CheckState::Info => ("INFO", BLUE),
        diagnostics::CheckState::Warn => ("WARN", GOLD),
        diagnostics::CheckState::Fail => ("FAIL", CORAL),
    };
    Line::from(vec![
        Span::styled(
            format!("  {marker:<4}  "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{}: {}", check.code, check.message)),
    ])
}

fn help_line<'a>(key: &'a str, description: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {key:<18}"), Style::default().fg(GOLD)),
        Span::raw(description),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn parse_process_names(value: &str) -> anyhow::Result<Vec<String>> {
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    collector::validate_watched_processes(&values)?;
    Ok(values)
}

fn signal_label(signal: SignalKind) -> &'static str {
    match signal {
        SignalKind::Cpu => "CPU utilization percentage",
        SignalKind::Memory => "Memory used and total bytes",
        SignalKind::Disk => "Aggregate disk used and total bytes",
        SignalKind::Process => "Exact process running state",
    }
}

fn short_uuid(value: Option<Uuid>) -> String {
    value.map_or_else(
        || "not assigned".to_owned(),
        |id| id.to_string()[..8].to_owned(),
    )
}

fn ratio(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        (used as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn format_bytes(value: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;
    let (amount, suffix) = if value >= TIB {
        (ratio_amount(value, TIB), "TiB")
    } else if value >= GIB {
        (ratio_amount(value, GIB), "GiB")
    } else if value >= MIB {
        (ratio_amount(value, MIB), "MiB")
    } else if value >= KIB {
        (ratio_amount(value, KIB), "KiB")
    } else {
        return format!("{value} B");
    };
    format!("{amount:.1} {suffix}")
}

fn ratio_amount(value: u64, divisor: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64 / divisor as f64
    }
}

fn mask_secret(length: usize) -> String {
    "*".repeat(length.min(64))
}

fn credential_is_fresh(value: Option<&str>) -> bool {
    value
        .and_then(|expiry| DateTime::parse_from_rfc3339(expiry).ok())
        .is_some_and(|expiry| expiry.with_timezone(&Utc) > Utc::now())
}

fn compact_timestamp(value: Option<&str>) -> String {
    value
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map_or_else(
            || "not recorded".to_owned(),
            |timestamp| {
                timestamp
                    .with_timezone(&Utc)
                    .format("%Y-%m-%d %H:%MZ")
                    .to_string()
            },
        )
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|value| value.kind() == io::ErrorKind::NotFound)
}

#[cfg(target_os = "windows")]
fn service_status() -> String {
    Command::new("schtasks.exe")
        .args(["/Query", "/TN", "Meerkateer Controller", "/FO", "LIST"])
        .output()
        .map_or_else(
            |_| "startup task unavailable".to_owned(),
            |output| {
                if output.status.success() {
                    "Windows startup task registered".to_owned()
                } else {
                    "Windows startup task not registered".to_owned()
                }
            },
        )
}

#[cfg(not(target_os = "windows"))]
fn service_status() -> String {
    Command::new("systemctl")
        .args(["is-active", "meerkateer-controller.service"])
        .output()
        .map_or_else(
            |_| "systemd unavailable".to_owned(),
            |output| {
                if output.status.success() {
                    "systemd service active".to_owned()
                } else {
                    "systemd service inactive or unavailable".to_owned()
                }
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_editor_trims_and_validates_exact_names() -> anyhow::Result<()> {
        assert_eq!(
            parse_process_names(" java, server.exe ")?,
            vec!["java", "server.exe"]
        );
        assert!(parse_process_names("java,JAVA").is_err());
        assert!(parse_process_names("token-worker").is_err());
        Ok(())
    }

    #[test]
    fn secret_mask_is_bounded_and_never_contains_secret() {
        assert_eq!(mask_secret(4), "****");
        assert_eq!(mask_secret(100).len(), 64);
    }

    #[test]
    fn byte_format_and_ratios_are_bounded() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert!(ratio(5, 0).abs() < f64::EPSILON);
        assert!((ratio(20, 10) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tab_navigation_wraps() {
        assert_eq!(Tab::Help.next(), Tab::Overview);
        assert_eq!(Tab::Overview.previous(), Tab::Help);
    }
}
