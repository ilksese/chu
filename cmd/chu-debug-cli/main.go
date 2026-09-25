package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"
)

const defaultURL = "http://127.0.0.1:17321/command"

type response struct {
	ID    string          `json:"id"`
	OK    bool            `json:"ok"`
	Data  json.RawMessage `json:"data"`
	Error string          `json:"error"`
}

type node struct {
	Ref      string `json:"ref"`
	Role     string `json:"role"`
	Name     string `json:"name"`
	Value    string `json:"value"`
	Disabled *bool  `json:"disabled"`
	Box      struct {
		X      int `json:"x"`
		Y      int `json:"y"`
		Width  int `json:"width"`
		Height int `json:"height"`
	} `json:"box"`
}

type snapshot struct {
	URL      string `json:"url"`
	Title    string `json:"title"`
	Viewport struct {
		Width  int `json:"width"`
		Height int `json:"height"`
	} `json:"viewport"`
	Nodes []node `json:"nodes"`
}

func main() {
	raw := flag.Bool("raw", false, "print only the JSON result")
	filename := flag.String("filename", "", "save screenshot to this file")
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "usage: chu-debug [--raw] [--filename=page.png] <command> [target] [value]\n")
	}
	flag.Parse()
	args := flag.Args()
	if len(args) == 0 {
		flag.Usage()
		os.Exit(2)
	}
	command := args[0]
	target := ""
	value := ""
	if len(args) > 1 {
		target = args[1]
	}
	if len(args) > 2 {
		value = strings.Join(args[2:], " ")
	}
	payload := commandPayload(command, target, value)
	if *filename != "" {
		payload["filename"] = *filename
	}
	result := post(payload)
	if command == "screenshot" && result.OK {
		saveScreenshot(result.Data, *filename)
	}
	if *raw {
		fmt.Println(string(result.Data))
		if !result.OK {
			os.Exit(1)
		}
		return
	}
	printResult(command, result)
	if !result.OK {
		os.Exit(1)
	}
}

func commandPayload(command, target, value string) map[string]any {
	payload := map[string]any{"id": fmt.Sprintf("%s-%d", command, time.Now().UnixNano()), "op": command}
	switch command {
	case "snapshot", "screenshot":
		if target != "" && !strings.HasPrefix(target, "e") {
			payload["selector"] = target
		} else if target != "" {
			payload["ref"] = target
		}
	case "find":
		payload["text"] = target
	case "goto":
		payload["op"] = "goto"
		payload["value"] = target
	case "click", "dblclick", "hover", "text", "html", "box", "check", "uncheck":
		setTarget(payload, target)
	case "fill", "select":
		setTarget(payload, target)
		payload["value"] = value
	case "press":
		payload["key"] = target
	case "type":
		payload["text"] = strings.TrimSpace(strings.Join(append([]string{target}, value), " "))
	case "scroll":
		payload["x"] = 0
		payload["y"] = 400
		if target != "" {
			var y int
			fmt.Sscan(target, &y)
			payload["y"] = y
		}
	case "eval":
		payload["script"] = strings.TrimSpace(strings.Join(append([]string{target}, value), " "))
	case "attr", "localstorage-get", "sessionstorage-get", "localstorage-delete", "sessionstorage-delete":
		if command == "attr" {
			setTarget(payload, target)
			payload["key"] = value
		} else {
			payload["key"] = target
		}
	case "localstorage-set", "sessionstorage-set":
		payload["key"] = target
		payload["value"] = value
	}
	return payload
}

func setTarget(payload map[string]any, target string) {
	if strings.HasPrefix(target, "e") && !strings.ContainsAny(target, " >#.[") {
		payload["ref"] = target
		return
	}
	payload["selector"] = target
}

func post(payload map[string]any) response {
	body, _ := json.Marshal(payload)
	req, err := http.NewRequest(http.MethodPost, defaultURL, bytes.NewReader(body))
	if err != nil {
		return response{Error: err.Error()}
	}
	req.Header.Set("Content-Type", "application/json")
	client := http.Client{Timeout: 12 * time.Second}
	res, err := client.Do(req)
	if err != nil {
		return response{Error: err.Error()}
	}
	defer res.Body.Close()
	raw, _ := io.ReadAll(res.Body)
	var result response
	if json.Unmarshal(raw, &result) != nil {
		return response{Error: string(raw)}
	}
	return result
}

func printResult(command string, result response) {
	if !result.OK {
		fmt.Fprintf(os.Stderr, "error: %s\n", result.Error)
		return
	}
	if command == "snapshot" || command == "click" || command == "dblclick" || command == "fill" || command == "check" || command == "uncheck" || command == "select" || command == "press" || command == "type" || command == "goto" {
		var page snapshot
		if json.Unmarshal(result.Data, &page) == nil && page.URL != "" {
			fmt.Printf("url: %s\ntitle: %s\nviewport: %dx%d\n", page.URL, page.Title, page.Viewport.Width, page.Viewport.Height)
			for _, item := range page.Nodes {
				extra := ""
				if item.Value != "" {
					extra = " value=" + item.Value
				}
				fmt.Printf("%s [%s] %s%s box=%d,%d %dx%d\n", item.Ref, item.Role, item.Name, extra, item.Box.X, item.Box.Y, item.Box.Width, item.Box.Height)
			}
			return
		}
	}
	var pretty bytes.Buffer
	if json.Indent(&pretty, result.Data, "", "  ") == nil {
		fmt.Println(pretty.String())
		return
	}
	fmt.Println(string(result.Data))
}

func saveScreenshot(data json.RawMessage, filename string) {
	var payload struct {
		Image string `json:"image"`
	}
	if json.Unmarshal(data, &payload) != nil || !strings.Contains(payload.Image, ",") {
		return
	}
	raw, err := base64.StdEncoding.DecodeString(strings.SplitN(payload.Image, ",", 2)[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "screenshot decode: %v\n", err)
		return
	}
	if filename == "" {
		filename = "chu-debug.png"
	}
	if err := os.WriteFile(filename, raw, 0o644); err != nil {
		fmt.Fprintf(os.Stderr, "screenshot write: %v\n", err)
		return
	}
	fmt.Printf("saved %s\n", filename)
}
