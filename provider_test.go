package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync/atomic"
	"testing"
)

func providerTestApp(t *testing.T) *App {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	return &App{root: filepath.Join(home, ".chu"), state: appState{Version: 1}}
}

func providerTestServer(t *testing.T, handler http.HandlerFunc) *httptest.Server {
	t.Helper()
	s := httptest.NewServer(handler)
	t.Cleanup(s.Close)
	return s
}

func TestProviderCRUDAndCredentialSources(t *testing.T) {
	a := providerTestApp(t)
	t.Setenv("CHU_PROVIDER_TEST_KEY", "process-secret")
	var calls atomic.Int32
	var expected atomic.Value
	expected.Store("Bearer file-secret")
	s := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		if r.URL.Path != "/v1/models" || r.Header.Get("Authorization") != expected.Load().(string) {
			t.Errorf("unexpected path/auth: %s / %q", r.URL.Path, r.Header.Get("Authorization"))
		}
		fmt.Fprint(w, `{"data":[{"id":"model-a"},{"id":"model-b"}]}`)
	})
	input := ProviderInput{Name: "  Example  ", EnvAPIKey: "CHU_PROVIDER_TEST_KEY", APIKey: "file-secret", BaseURL: s.URL + "/v1/"}
	snapshot, err := a.CreateProvider(input)
	if err != nil || len(snapshot.Providers) != 1 {
		t.Fatalf("create: %#v %v", snapshot.Providers, err)
	}
	p := snapshot.Providers[0]
	if p.Name != "Example" || p.ID == "" || p.ModelsFetchedAt == "" || !reflect.DeepEqual(p.Models, []string{"model-a", "model-b"}) {
		t.Fatalf("provider: %#v", p)
	}
	input.Name, input.APIKey = "Renamed", ""
	snapshot, err = a.UpdateProvider(p.ID, input)
	if err != nil || snapshot.Providers[0].ID != p.ID || calls.Load() != 1 {
		t.Fatalf("rename: %#v %v calls=%d", snapshot.Providers, err, calls.Load())
	}
	if os.Getenv(input.EnvAPIKey) != "process-secret" {
		t.Fatal("process environment changed")
	}
	state, err := os.ReadFile(filepath.Join(a.root, "state.json"))
	if err != nil || strings.Contains(string(state), "file-secret") {
		t.Fatalf("state contains credential or unreadable: %v", err)
	}
	encoded, _ := json.Marshal(snapshot)
	if strings.Contains(string(encoded), "file-secret") || strings.Contains(string(encoded), `"apiKey"`) {
		t.Fatal("snapshot exposes credentials")
	}
	// Changing the reference must not copy the old key; missing references never send anonymously.
	input.EnvAPIKey = "CHU_PROVIDER_MISSING_KEY"
	t.Setenv(input.EnvAPIKey, "")
	snapshot, err = a.UpdateProvider(p.ID, input)
	if err != nil || snapshot.Providers[0].ModelsError == "" || calls.Load() != 1 || len(snapshot.Providers[0].Models) != 0 {
		t.Fatalf("missing reference: %#v %v", snapshot.Providers, err)
	}
	expected.Store("")
	input.EnvAPIKey = ""
	if snapshot, err = a.UpdateProvider(p.ID, input); err != nil || snapshot.Providers[0].ModelsError != "" {
		t.Fatalf("anonymous: %#v %v", snapshot.Providers, err)
	}
	if _, err := a.DeleteProvider(p.ID); err != nil {
		t.Fatal(err)
	}
	if key, err := providerKey(a.root, "CHU_PROVIDER_TEST_KEY"); err != nil || key != "file-secret" {
		t.Fatalf("deleted credential not retained: %q %v", key, err)
	}
	snapshot = a.snapshotLocked()
	if snapshot.Providers == nil || len(snapshot.Providers) != 0 {
		t.Fatal("providers must be an empty array")
	}
	entries, err := os.ReadDir(filepath.Dir(a.root))
	if err != nil || len(entries) != 1 || entries[0].Name() != ".chu" {
		t.Fatalf("host paths touched: %v %v", entries, err)
	}
	// File removal restores process-environment fallback.
	if err := os.Remove(filepath.Join(a.root, ".env")); err != nil {
		t.Fatal(err)
	}
	if key, err := providerKey(a.root, "CHU_PROVIDER_TEST_KEY"); err != nil || key != "process-secret" {
		t.Fatalf("process fallback: %q %v", key, err)
	}
}

func TestProviderValidation(t *testing.T) {
	a := providerTestApp(t)
	s := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, `{"data":[]}`) })
	valid := ProviderInput{Name: "Example", BaseURL: s.URL, EnvAPIKey: "CHU_TEST_VALIDATION"}
	t.Setenv(valid.EnvAPIKey, "")
	if _, err := a.CreateProvider(valid); err != nil {
		t.Fatal(err)
	}
	for _, input := range []ProviderInput{
		{Name: "example", BaseURL: s.URL},
		{Name: "Another", BaseURL: s.URL, EnvAPIKey: valid.EnvAPIKey},
		{Name: " ", BaseURL: s.URL},
		{Name: "Another", BaseURL: "ftp://example.com"},
		{Name: "Another", BaseURL: "https://user:pass@example.com"},
		{Name: "Another", BaseURL: "https://example.com?secret=value"},
		{Name: "Another", BaseURL: s.URL, EnvAPIKey: "INVALID-KEY"},
		{Name: "Another", BaseURL: s.URL, EnvAPIKey: " KEY "},
		{Name: "Another", BaseURL: s.URL, APIKey: "secret"},
	} {
		if _, err := a.CreateProvider(input); err == nil {
			t.Errorf("accepted invalid input: %#v", input)
		}
	}
	if len(a.state.Providers) != 1 {
		t.Fatal("validation changed state")
	}
}

func TestProviderModelsFailuresPreserveCache(t *testing.T) {
	a := providerTestApp(t)
	var body atomic.Value
	body.Store(`{"data":[{"id":"cached"}]}`)
	var status atomic.Int32
	status.Store(200)
	s := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(int(status.Load()))
		fmt.Fprint(w, body.Load().(string))
	})
	snapshot, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: s.URL})
	if err != nil {
		t.Fatal(err)
	}
	p := snapshot.Providers[0]
	for _, invalid := range []string{`not-json-secret`, `{}`, `{"data":null}`, `{"data":{}}`, `{"data":[{}]}`, `{"data":[{"id":123}]}`, strings.Repeat("x", (4<<20)+1)} {
		body.Store(invalid)
		snapshot, err = a.RefreshProviderModels(p.ID)
		got := snapshot.Providers[0]
		if err != nil || got.ModelsError == "" || !reflect.DeepEqual(got.Models, p.Models) || got.ModelsFetchedAt != p.ModelsFetchedAt || strings.Contains(got.ModelsError, "secret") {
			t.Fatalf("failure lost cache/exposed body: %#v %v", got, err)
		}
	}
	status.Store(401)
	snapshot, err = a.RefreshProviderModels(p.ID)
	if err != nil || !strings.Contains(snapshot.Providers[0].ModelsError, "401") {
		t.Fatalf("HTTP error: %#v %v", snapshot.Providers, err)
	}
	// Connection edits invalidate the old successful cache even when the next fetch fails.
	snapshot, err = a.UpdateProvider(p.ID, ProviderInput{Name: p.Name, BaseURL: s.URL + "/new"})
	if err != nil || len(snapshot.Providers[0].Models) != 0 || snapshot.Providers[0].ModelsFetchedAt != "" || snapshot.Providers[0].ModelsError == "" {
		t.Fatalf("connection update: %#v %v", snapshot.Providers, err)
	}
	reloaded := &App{root: a.root}
	if err := reloaded.loadState(); err != nil || reloaded.state.Providers[0].ModelsError == "" {
		t.Fatalf("error not persisted: %v", err)
	}
	status.Store(200)
	body.Store(`{"data":[]}`)
	snapshot, err = a.RefreshProviderModels(p.ID)
	if err != nil || snapshot.Providers[0].Models == nil || snapshot.Providers[0].ModelsError != "" || snapshot.Providers[0].ModelsFetchedAt == "" {
		t.Fatalf("empty success: %#v %v", snapshot.Providers, err)
	}
}

func TestProviderRedirectDoesNotLeakKey(t *testing.T) {
	a := providerTestApp(t)
	var calls atomic.Int32
	target := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) { calls.Add(1) })
	source := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) { http.Redirect(w, r, target.URL, http.StatusFound) })
	snapshot, err := a.CreateProvider(ProviderInput{Name: "Redirect", BaseURL: source.URL, EnvAPIKey: "CHU_REDIRECT_KEY", APIKey: "secret"})
	if err != nil || calls.Load() != 0 || !strings.Contains(snapshot.Providers[0].ModelsError, "302") {
		t.Fatalf("redirect followed: %#v %v calls=%d", snapshot.Providers, err, calls.Load())
	}
}

func TestProviderStaleRequestDoesNotOverwrite(t *testing.T) {
	for _, action := range []string{"edit", "delete", "refresh"} {
		t.Run(action, func(t *testing.T) {
			a := providerTestApp(t)
			started, release := make(chan struct{}), make(chan struct{})
			var calls atomic.Int32
			s := providerTestServer(t, func(w http.ResponseWriter, r *http.Request) {
				if calls.Add(1) == 2 {
					close(started)
					<-release
					fmt.Fprint(w, `{"data":[{"id":"stale"}]}`)
					return
				}
				fmt.Fprint(w, `{"data":[{"id":"current"}]}`)
			})
			snapshot, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: s.URL})
			if err != nil {
				t.Fatal(err)
			}
			id := snapshot.Providers[0].ID
			done := make(chan error, 1)
			go func() { _, err := a.RefreshProviderModels(id); done <- err }()
			<-started
			switch action {
			case "edit":
				_, err = a.UpdateProvider(id, ProviderInput{Name: "Changed", BaseURL: s.URL + "/new"})
			case "delete":
				_, err = a.DeleteProvider(id)
			case "refresh":
				_, err = a.RefreshProviderModels(id)
			}
			close(release)
			if err != nil {
				t.Fatal(err)
			}
			if err := <-done; err != nil {
				t.Fatal(err)
			}
			if action == "delete" {
				if len(a.state.Providers) != 0 {
					t.Fatal("deleted provider resurrected")
				}
			} else if a.state.Providers[0].Models[0] != "current" {
				t.Fatal("stale response overwrote current models")
			}
		})
	}
}
