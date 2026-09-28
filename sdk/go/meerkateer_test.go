package meerkateer

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestHeartbeatRetryKeepsIdentity(t *testing.T) {
	var id, body string
	calls := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls++
		defer r.Body.Close()
		var payload map[string]any
		if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
			t.Fatal(err)
		}
		if r.URL.Path != "/v1/ingest/heartbeat" || payload["status"] != "ok" {
			t.Fatal("invalid heartbeat")
		}
		currentID := r.Header.Get("Idempotency-Key")
		encoded, _ := json.Marshal(payload)
		if calls == 1 {
			id, body = currentID, string(encoded)
			w.WriteHeader(503)
			return
		}
		if id != currentID || body != string(encoded) {
			t.Fatal("retry changed fact identity")
		}
		w.WriteHeader(202)
		_ = json.NewEncoder(w).Encode(map[string]string{"idempotency_key": id})
	}))
	defer server.Close()
	client, err := New(server.URL, "mks_sk_fixture123", "store", "worker-01", "test")
	if err != nil {
		t.Fatal(err)
	}
	if err := client.Heartbeat(context.Background(), "ok", "worker ready"); err != nil {
		t.Fatal(err)
	}
	if calls != 2 {
		t.Fatalf("expected 2 attempts, got %d", calls)
	}
}

func TestRejectsUnsafeURLAndRedirect(t *testing.T) {
	if _, err := New("http://example.com", "mks_sk_fixture123", "store", "worker-01", "test"); err == nil {
		t.Fatal("accepted HTTP remote origin")
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Location", "https://example.com")
		w.WriteHeader(302)
	}))
	defer server.Close()
	client, err := New(server.URL, "mks_sk_fixture123", "store", "worker-01", "test")
	if err != nil {
		t.Fatal(err)
	}
	if err := client.Heartbeat(context.Background(), "ok", ""); err == nil {
		t.Fatal("accepted redirect")
	}
}
