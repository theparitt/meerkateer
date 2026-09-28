// Package meerkateer sends bounded MKS-1 signals from Go services.
package meerkateer

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"regexp"
	"strings"
	"time"
)

var servicePattern = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`)
var projectPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]{0,63}$`)
var kindPattern = regexp.MustCompile(`^[a-z0-9]+(_[a-z0-9]+)+$`)
var uuidPattern = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)

// Client is bound to a single process identity and service credential.
type Client struct {
	url, key, project, service, environment string
	http                                    *http.Client
}

// New validates the server origin and identity before accepting telemetry.
func New(rawURL, key, project, service, environment string) (*Client, error) {
	u, err := url.Parse(rawURL)
	if err != nil || u.Hostname() == "" || (u.Scheme != "http" && u.Scheme != "https") || u.User != nil || u.RawQuery != "" || u.Fragment != "" || (u.Path != "" && u.Path != "/") {
		return nil, errors.New("Meerkateer URL must be an HTTP(S) origin without credentials or a path")
	}
	host := u.Hostname()
	ip := net.ParseIP(host)
	if u.Scheme != "https" && host != "localhost" && (ip == nil || !ip.IsLoopback()) {
		return nil, errors.New("HTTPS is required except for loopback development URLs")
	}
	if !strings.HasPrefix(key, "mks_sk_") || len(key) < 8 || !projectPattern.MatchString(project) || !servicePattern.MatchString(service) {
		return nil, errors.New("Meerkateer service identity is invalid")
	}
	switch environment {
	case "development", "staging", "production", "test", "local":
	default:
		return nil, errors.New("Meerkateer environment is invalid")
	}
	return &Client{
		url: u.Scheme + "://" + u.Host + "/", key: key, project: project, service: service, environment: environment,
		http: &http.Client{Timeout: 5 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }},
	}, nil
}

// FromEnv reads the one-time service identity shown by the Console.
func FromEnv() (*Client, error) {
	environment := os.Getenv("MEERKATEER_ENVIRONMENT")
	if environment == "" {
		environment = "production"
	}
	return New(os.Getenv("MEERKATEER_URL"), os.Getenv("MEERKATEER_SERVICE_KEY"), os.Getenv("MEERKATEER_PROJECT"), os.Getenv("MEERKATEER_SERVICE"), environment)
}

// Heartbeat reports the current process status. Keep messages free of secrets and user data.
func (c *Client) Heartbeat(ctx context.Context, status, message string) error {
	if status != "ok" && status != "degraded" && status != "down" {
		return errors.New("heartbeat status is invalid")
	}
	return c.send(ctx, "heartbeat", map[string]any{"status": status, "message": message}, "")
}

// Event reports a stable operational event kind such as queue_delay.
func (c *Client) Event(ctx context.Context, kind, level, message string, count int) error {
	if !kindPattern.MatchString(kind) || len(kind) > 128 {
		return errors.New("event kind is invalid")
	}
	if level != "info" && level != "warning" && level != "error" && level != "critical" {
		return errors.New("event level is invalid")
	}
	if count < 1 || count > 1_000_000 {
		return errors.New("event count is invalid")
	}
	return c.send(ctx, "event", map[string]any{"kind": kind, "level": level, "message": message, "count": count}, "")
}

// Deploy reports a deployment transition.
func (c *Client) Deploy(ctx context.Context, version, commit, status string) error {
	if len(version) < 1 || len(version) > 128 || len(commit) < 1 || len(commit) > 128 {
		return errors.New("deployment identity is invalid")
	}
	if status != "started" && status != "finished" && status != "failed" {
		return errors.New("deployment status is invalid")
	}
	return c.send(ctx, "deploy", map[string]any{"version": version, "commit": commit, "status": status}, "")
}

// HeartbeatWithID sends a caller-owned fact ID for durable retries across restarts.
func (c *Client) HeartbeatWithID(ctx context.Context, status, message, id string) error {
	if status != "ok" && status != "degraded" && status != "down" {
		return errors.New("heartbeat status is invalid")
	}
	return c.send(ctx, "heartbeat", map[string]any{"status": status, "message": message}, id)
}

func newID() (string, error) {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", errors.New("could not create telemetry ID")
	}
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	s := hex.EncodeToString(b[:])
	return s[:8] + "-" + s[8:12] + "-" + s[12:16] + "-" + s[16:20] + "-" + s[20:], nil
}

func (c *Client) send(ctx context.Context, kind string, fields map[string]any, id string) error {
	if id == "" {
		var err error
		id, err = newID()
		if err != nil {
			return err
		}
	}
	if !uuidPattern.MatchString(id) {
		return errors.New("idempotency key must be a UUID")
	}
	if message, ok := fields["message"].(string); ok && len([]rune(message)) > 1024 {
		return errors.New("message is too long")
	}
	fields["interface_version"] = "1"
	fields["project"] = c.project
	fields["service"] = c.service
	fields["environment"] = c.environment
	fields["timestamp"] = time.Now().UTC().Format(time.RFC3339Nano)
	body, err := json.Marshal(fields)
	if err != nil || len(body) > 16*1024 {
		return errors.New("telemetry payload is invalid or too large")
	}
	for attempt := 0; attempt <= 2; attempt++ {
		req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.url+"v1/ingest/"+kind, bytes.NewReader(body))
		if err != nil {
			return errors.New("could not build telemetry request")
		}
		req.Header.Set("Authorization", "Bearer "+c.key)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Accept", "application/json")
		req.Header.Set("Idempotency-Key", id)
		response, sendErr := c.http.Do(req)
		if sendErr == nil {
			limited, readErr := io.ReadAll(io.LimitReader(response.Body, 64*1024+1))
			response.Body.Close()
			if readErr != nil || len(limited) > 64*1024 {
				return errors.New("Meerkateer acknowledgement is invalid")
			}
			if response.StatusCode == http.StatusOK || response.StatusCode == http.StatusAccepted {
				var acknowledgement struct {
					IdempotencyKey string `json:"idempotency_key"`
				}
				if json.Unmarshal(limited, &acknowledgement) != nil || acknowledgement.IdempotencyKey != id {
					return errors.New("Meerkateer acknowledgement key mismatch")
				}
				return nil
			}
			if response.StatusCode != http.StatusTooManyRequests && response.StatusCode < 500 {
				return fmt.Errorf("Meerkateer rejected telemetry with HTTP %d", response.StatusCode)
			}
		}
		if attempt == 2 {
			break
		}
		select {
		case <-ctx.Done():
			return errors.New("telemetry delivery cancelled")
		case <-time.After(time.Duration(250*(1<<attempt)) * time.Millisecond):
		}
	}
	return errors.New("telemetry delivery failed after bounded retries")
}
