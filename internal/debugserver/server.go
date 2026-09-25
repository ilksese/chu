package debugserver

import (
	"encoding/json"
	"log"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

type debugClient struct {
	conn *websocket.Conn
	send chan []byte
}

type debugResult struct {
	ID    string          `json:"id"`
	OK    bool            `json:"ok"`
	Data  json.RawMessage `json:"data"`
	Error string          `json:"error"`
}

type debugHub struct {
	mu      sync.Mutex
	clients map[*debugClient]struct{}
	waiters map[string]chan debugResult
}

func (h *debugHub) add(client *debugClient) {
	h.mu.Lock()
	h.clients[client] = struct{}{}
	h.mu.Unlock()
	log.Printf("debug page connected (%d)", len(h.clients))
}

func (h *debugHub) remove(client *debugClient) {
	h.mu.Lock()
	delete(h.clients, client)
	h.mu.Unlock()
}

func (h *debugHub) broadcast(message []byte) int {
	h.mu.Lock()
	defer h.mu.Unlock()
	for client := range h.clients {
		select {
		case client.send <- message:
		default:
		}
	}
	return len(h.clients)
}

func (h *debugHub) deliver(result debugResult) {
	h.mu.Lock()
	waiter := h.waiters[result.ID]
	delete(h.waiters, result.ID)
	h.mu.Unlock()
	if waiter != nil {
		waiter <- result
	}
}

func (h *debugHub) command(body json.RawMessage) (debugResult, error) {
	var request struct {
		ID string `json:"id"`
	}
	if err := json.Unmarshal(body, &request); err != nil {
		return debugResult{}, err
	}
	waiter := make(chan debugResult, 1)
	h.mu.Lock()
	h.waiters[request.ID] = waiter
	h.mu.Unlock()
	if h.broadcast(body) == 0 {
		h.mu.Lock()
		delete(h.waiters, request.ID)
		h.mu.Unlock()
		return debugResult{ID: request.ID, Error: "no page connected"}, nil
	}
	select {
	case result := <-waiter:
		return result, nil
	case <-time.After(8 * time.Second):
		h.mu.Lock()
		delete(h.waiters, request.ID)
		h.mu.Unlock()
		return debugResult{ID: request.ID, Error: "timeout"}, nil
	}
}

func Start(addr string) {
	hub := &debugHub{
		clients: map[*debugClient]struct{}{},
		waiters: map[string]chan debugResult{},
	}
	upgrader := websocket.Upgrader{CheckOrigin: func(*http.Request) bool { return true }}
	mux := http.NewServeMux()
	mux.HandleFunc("/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("debug upgrade: %v", err)
			return
		}
		client := &debugClient{conn: conn, send: make(chan []byte, 16)}
		hub.add(client)
		defer hub.remove(client)
		defer conn.Close()
		go func() {
			for message := range client.send {
				if err := conn.WriteMessage(websocket.TextMessage, message); err != nil {
					return
				}
			}
		}()
		for {
			_, message, err := conn.ReadMessage()
			if err != nil {
				close(client.send)
				return
			}
			var result debugResult
			if json.Unmarshal(message, &result) == nil && result.ID != "" {
				hub.deliver(result)
				continue
			}
			log.Printf("debug page: %s", message)
		}
	})
	mux.HandleFunc("/command", func(w http.ResponseWriter, r *http.Request) {
		var body json.RawMessage
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		result, err := hub.command(body)
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(result)
	})
	log.Printf("debug server listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}
