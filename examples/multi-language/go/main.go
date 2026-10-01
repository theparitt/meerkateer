package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"strconv"
	"sync"
	"time"

	meerkateer "github.com/theparitt/meerkateer/sdk/go"
)

const language = "go"

type demo struct {
	client *meerkateer.Client
	mu     sync.RWMutex
	state  string
}

func (d *demo) current() string {
	d.mu.RLock()
	defer d.mu.RUnlock()
	return d.state
}

func (d *demo) signal(ctx context.Context, next string) error {
	messages := map[string]string{
		"ok": "demo service recovered", "degraded": "demo dependency is slow", "down": "demo process is unavailable",
	}
	d.mu.Lock()
	previous := d.state
	d.state = next
	d.mu.Unlock()
	if err := d.client.Heartbeat(ctx, next, messages[next]); err != nil {
		return err
	}
	if previous == next {
		return nil
	}
	levels := map[string]string{"ok": "info", "degraded": "warning", "down": "error"}
	kinds := map[string]string{"ok": "demo_recovered", "degraded": "demo_degraded", "down": "demo_failed"}
	return d.client.Event(ctx, kinds[next], levels[next], messages[next], 1)
}

func reply(response http.ResponseWriter, status int, payload map[string]string) {
	response.Header().Set("Content-Type", "application/json")
	response.WriteHeader(status)
	if err := json.NewEncoder(response).Encode(payload); err != nil {
		log.Printf("response write failed: %v", err)
	}
}

func main() {
	client, err := meerkateer.FromEnv()
	if err != nil {
		log.Fatal(err)
	}
	d := &demo{client: client, state: "ok"}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	if err = client.Deploy(ctx, "demo-1", "local", "finished"); err == nil {
		err = d.signal(ctx, "ok")
	}
	cancel()
	if err != nil {
		log.Fatal(err)
	}
	intervalSeconds, err := strconv.Atoi(env("DEMO_HEARTBEAT_SECONDS", "5"))
	if err != nil || intervalSeconds < 1 {
		log.Fatal("DEMO_HEARTBEAT_SECONDS must be a positive integer")
	}
	go func() {
		ticker := time.NewTicker(time.Duration(intervalSeconds) * time.Second)
		defer ticker.Stop()
		for range ticker.C {
			beatCtx, beatCancel := context.WithTimeout(context.Background(), 10*time.Second)
			err := client.Heartbeat(beatCtx, d.current(), language+" demo heartbeat")
			beatCancel()
			if err != nil {
				log.Printf("heartbeat delivery failed: %v", err)
			}
		}
	}()

	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", func(response http.ResponseWriter, _ *http.Request) {
		reply(response, http.StatusOK, map[string]string{"language": language, "status": d.current()})
	})
	for path, next := range map[string]string{"healthy": "ok", "degraded": "degraded", "down": "down"} {
		next := next
		mux.HandleFunc("POST /scenario/"+path, func(response http.ResponseWriter, request *http.Request) {
			if err := d.signal(request.Context(), next); err != nil {
				reply(response, http.StatusBadGateway, map[string]string{"error": "telemetry_failed", "detail": err.Error()})
				return
			}
			reply(response, http.StatusOK, map[string]string{"language": language, "status": next})
		})
	}
	address := "127.0.0.1:" + env("DEMO_PORT", "19103")
	log.Printf("%s demo listening on http://%s", language, address)
	server := &http.Server{Addr: address, Handler: mux, ReadHeaderTimeout: 5 * time.Second}
	if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatal(fmt.Errorf("HTTP server failed: %w", err))
	}
}

func env(name, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}
