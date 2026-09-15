package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/pelletier/go-toml/v2"
)

type App struct {
	ctx   context.Context
	mu    sync.Mutex
	root  string
	state appState
}

type appState struct {
	Version int                 `json:"version"`
	Paths   map[string]hostPath `json:"paths,omitempty"`
	Skills  []storedSkill       `json:"skills"`
	MCPs    []storedMCP         `json:"mcps"`
	Agents  []storedAgent       `json:"agents"`
}

type hostPath struct {
	Config string `json:"config"`
	Skills string `json:"skills"`
	Agents string `json:"agents"`
}

type deployment struct {
	Enabled  bool   `json:"enabled"`
	Mode     string `json:"mode,omitempty"`
	Target   string `json:"target,omitempty"`
	LastHash string `json:"lastHash,omitempty"`
}

type storedSkill struct {
	ID          string                `json:"id"`
	Name        string                `json:"name"`
	Description string                `json:"description,omitempty"`
	Repository  string                `json:"repository,omitempty"`
	SourceDir   string                `json:"sourceDir"`
	Version     string                `json:"version,omitempty"`
	Deployments map[string]deployment `json:"deployments,omitempty"`
}

type storedMCP struct {
	ID          string                `json:"id"`
	Name        string                `json:"name"`
	Description string                `json:"description,omitempty"`
	Type        string                `json:"type"`
	Endpoint    string                `json:"endpoint,omitempty"`
	Command     string                `json:"command,omitempty"`
	Args        []string              `json:"args,omitempty"`
	Env         map[string]string     `json:"env,omitempty"`
	Headers     map[string]string     `json:"headers,omitempty"`
	Secret      string                `json:"secret,omitempty"`
	Deployments map[string]deployment `json:"deployments,omitempty"`
}

type storedAgent struct {
	ID          string                `json:"id"`
	Name        string                `json:"name"`
	Description string                `json:"description,omitempty"`
	Prompt      string                `json:"prompt"`
	Model       string                `json:"model,omitempty"`
	Source      string                `json:"source"`
	Deployments map[string]deployment `json:"deployments,omitempty"`
}

type HostView struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Installed   bool   `json:"installed"`
	Status      string `json:"status"`
	ConfigPath  string `json:"configPath"`
	SkillPath   string `json:"skillPath"`
	AgentPath   string `json:"agentPath"`
	Format      string `json:"format"`
}

type SkillView struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Repository  string            `json:"repository"`
	Version     string            `json:"version"`
	Source      string            `json:"source"`
	Managed     bool              `json:"managed"`
	EnabledOn   map[string]bool   `json:"enabledOn"`
	ModeByHost  map[string]string `json:"modeByHost"`
}

type MCPView struct {
	ID             string          `json:"id"`
	Name           string          `json:"name"`
	Description    string          `json:"description"`
	Type           string          `json:"type"`
	Endpoint       string          `json:"endpoint"`
	Command        string          `json:"command"`
	HasCredentials bool            `json:"hasCredentials"`
	Managed        bool            `json:"managed"`
	EnabledOn      map[string]bool `json:"enabledOn"`
}

type AgentView struct {
	ID          string          `json:"id"`
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Model       string          `json:"model"`
	Source      string          `json:"source"`
	Managed     bool            `json:"managed"`
	EnabledOn   map[string]bool `json:"enabledOn"`
}

type Snapshot struct {
	Root     string      `json:"root"`
	Hosts    []HostView  `json:"hosts"`
	Skills   []SkillView `json:"skills"`
	MCPs     []MCPView   `json:"mcps"`
	Agents   []AgentView `json:"agents"`
	LastScan string      `json:"lastScan"`
}

type MCPInput struct {
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Type        string            `json:"type"`
	Endpoint    string            `json:"endpoint"`
	Command     string            `json:"command"`
	Args        []string          `json:"args"`
	Env         map[string]string `json:"env"`
	Headers     map[string]string `json:"headers"`
	Secret      string            `json:"secret"`
}

type AgentInput struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Prompt      string `json:"prompt"`
	Model       string `json:"model"`
}

type hostSpec struct {
	id          string
	name        string
	description string
	config      []string
	skills      []string
	agents      []string
}

func NewApp() *App {
	home, err := os.UserHomeDir()
	if err != nil {
		home = "."
	}
	a := &App{root: filepath.Join(home, ".chu"), state: appState{Version: 1}}
	_ = a.loadState()
	return a
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
}

func (a *App) GetSnapshot() Snapshot {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.snapshotLocked()
}

func (a *App) Refresh() Snapshot {
	return a.GetSnapshot()
}

func (a *App) InstallSkill(repository, subdir, name string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if !isHTTPS(repository) {
		return a.snapshotLocked(), errors.New("skill 来源必须是公开 HTTPS Git 仓库")
	}
	if name == "" {
		name = filepath.Base(strings.TrimSuffix(repository, "/"))
		name = strings.TrimSuffix(name, ".git")
	}
	name = slug(name)
	if name == "" {
		return a.snapshotLocked(), errors.New("无法从仓库地址确定 skill 名称")
	}
	for _, item := range a.state.Skills {
		if item.Name == name {
			return a.snapshotLocked(), fmt.Errorf("skill %q 已存在", name)
		}
	}

	cacheDir := filepath.Join(a.root, "sources", name)
	if err := os.RemoveAll(cacheDir); err != nil {
		return a.snapshotLocked(), err
	}
	if err := os.MkdirAll(filepath.Dir(cacheDir), 0o700); err != nil {
		return a.snapshotLocked(), err
	}
	cmd := exec.Command("git", "clone", "--depth", "1", repository, cacheDir)
	if output, err := cmd.CombinedOutput(); err != nil {
		return a.snapshotLocked(), fmt.Errorf("Git clone 失败: %s", strings.TrimSpace(string(output)))
	}
	source := filepath.Join(cacheDir, filepath.Clean(subdir))
	if subdir == "" || subdir == "." {
		source = cacheDir
	}
	if !fileExists(filepath.Join(source, "SKILL.md")) {
		return a.snapshotLocked(), errors.New("所选目录中没有 SKILL.md")
	}
	target := filepath.Join(a.root, "skills", name)
	if fileExists(target) {
		return a.snapshotLocked(), fmt.Errorf("中央 skill 目录已存在: %s", target)
	}
	if err := copyDir(source, target); err != nil {
		return a.snapshotLocked(), err
	}
	a.state.Skills = append(a.state.Skills, storedSkill{
		ID: slug(name), Name: name, Description: readSkillDescription(source),
		Repository: repository, SourceDir: target, Version: "main", Deployments: map[string]deployment{},
	})
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ImportSkill(hostID, name string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	host, ok := a.hostByID(hostID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	source := filepath.Join(host.SkillPath, name)
	if !fileExists(filepath.Join(source, "SKILL.md")) {
		return a.snapshotLocked(), errors.New("未找到可导入的 SKILL.md")
	}
	for _, item := range a.state.Skills {
		if item.Name == name {
			return a.snapshotLocked(), fmt.Errorf("skill %q 已存在", name)
		}
	}
	target := filepath.Join(a.root, "skills", slug(name))
	if fileExists(target) {
		return a.snapshotLocked(), fmt.Errorf("中央 skill 目录已存在: %s", target)
	}
	if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
		return a.snapshotLocked(), err
	}
	if err := os.Rename(source, target); err != nil {
		return a.snapshotLocked(), fmt.Errorf("移动 skill 失败: %w", err)
	}
	mode := "link"
	if err := os.Symlink(target, source); err != nil {
		if copyErr := copyDir(target, source); copyErr != nil {
			_ = os.Rename(target, source)
			return a.snapshotLocked(), fmt.Errorf("link 和 copy 都失败: %w", copyErr)
		}
		mode = "copy"
	}
	item := storedSkill{ID: slug(name), Name: name, Description: readSkillDescription(target), SourceDir: target, Version: "imported", Deployments: map[string]deployment{}}
	item.Deployments[hostID] = deployment{Enabled: true, Mode: mode, Target: source, LastHash: hashPath(source)}
	a.state.Skills = append(a.state.Skills, item)
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ToggleSkill(skillID, hostID string, enabled bool) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	item, ok := a.skillByID(skillID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 skill")
	}
	host, ok := a.hostByID(hostID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if item.Deployments == nil {
		item.Deployments = map[string]deployment{}
	}
	dep := item.Deployments[hostID]
	target := filepath.Join(host.SkillPath, item.Name)
	if enabled {
		if !fileExists(filepath.Join(item.SourceDir, "SKILL.md")) {
			return a.snapshotLocked(), errors.New("中央 skill 源目录不可用")
		}
		if dep.Enabled {
			return a.snapshotLocked(), nil
		}
		if fileExists(target) {
			return a.snapshotLocked(), fmt.Errorf("目标目录已存在，请先处理冲突: %s", target)
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
			return a.snapshotLocked(), err
		}
		mode := "link"
		if err := os.Symlink(item.SourceDir, target); err != nil {
			if copyErr := copyDir(item.SourceDir, target); copyErr != nil {
				return a.snapshotLocked(), fmt.Errorf("link 失败，copy 也失败: %w", copyErr)
			}
			mode = "copy"
		}
		item.Deployments[hostID] = deployment{Enabled: true, Mode: mode, Target: target, LastHash: hashPath(target)}
	} else {
		if !dep.Enabled {
			return a.snapshotLocked(), nil
		}
		if dep.Mode == "copy" {
			if current := hashPath(target); current != dep.LastHash {
				return a.snapshotLocked(), errors.New("目标 copy 已被修改，已保护用户改动")
			}
			if err := os.RemoveAll(target); err != nil {
				return a.snapshotLocked(), err
			}
		} else if fileExists(target) {
			resolved, err := filepath.EvalSymlinks(target)
			if err != nil || filepath.Clean(resolved) != filepath.Clean(item.SourceDir) {
				return a.snapshotLocked(), errors.New("目标 link 已被修改，已保护用户改动")
			}
			if err := os.Remove(target); err != nil {
				return a.snapshotLocked(), err
			}
		}
		delete(item.Deployments, hostID)
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) AddMCP(input MCPInput) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	input.Name = strings.TrimSpace(input.Name)
	input.Type = strings.ToLower(strings.TrimSpace(input.Type))
	if input.Name == "" {
		return a.snapshotLocked(), errors.New("MCP 名称不能为空")
	}
	if input.Type != "stdio" && input.Type != "http" && input.Type != "sse" {
		return a.snapshotLocked(), errors.New("MCP 类型必须是 stdio、http 或 sse")
	}
	if input.Type == "stdio" && strings.TrimSpace(input.Command) == "" {
		return a.snapshotLocked(), errors.New("stdio MCP 必须填写启动命令")
	}
	if input.Type != "stdio" && strings.TrimSpace(input.Endpoint) == "" {
		return a.snapshotLocked(), errors.New("HTTP/SSE MCP 必须填写 URL")
	}
	if input.Type == "stdio" {
		command, commandArgs := splitCommandLine(input.Command)
		input.Command = command
		input.Args = append(commandArgs, input.Args...)
	}
	for _, item := range a.state.MCPs {
		if item.Name == input.Name {
			return a.snapshotLocked(), fmt.Errorf("MCP %q 已存在", input.Name)
		}
	}
	a.state.MCPs = append(a.state.MCPs, storedMCP{
		ID: slug(input.Name), Name: input.Name, Description: input.Description, Type: input.Type,
		Endpoint: input.Endpoint, Command: input.Command, Args: input.Args, Env: input.Env,
		Headers: input.Headers, Secret: input.Secret, Deployments: map[string]deployment{},
	})
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ToggleMCP(mcpID, hostID string, enabled bool) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	item, ok := a.mcpByID(mcpID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 MCP")
	}
	host, ok := a.hostByID(hostID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if item.Deployments == nil {
		item.Deployments = map[string]deployment{}
	}
	dep := item.Deployments[hostID]
	if err := a.validateMCPConfigHash(hostID, host.ConfigPath); err != nil {
		return a.snapshotLocked(), err
	}
	if !fileExists(host.ConfigPath) {
		if err := os.MkdirAll(filepath.Dir(host.ConfigPath), 0o700); err != nil {
			return a.snapshotLocked(), err
		}
	}
	root, err := readConfig(host.ConfigPath, host.Format)
	if err != nil {
		return a.snapshotLocked(), fmt.Errorf("无法解析宿主配置: %w", err)
	}
	key := mcpRootKey(host.ID)
	servers := ensureMap(root, key)
	if enabled {
		if dep.Enabled {
			return a.snapshotLocked(), nil
		}
		if _, exists := servers[item.Name]; exists {
			return a.snapshotLocked(), fmt.Errorf("目标配置中已存在同名 MCP: %s", item.Name)
		}
		servers[item.Name] = itemConfig(*item)
	} else {
		if !dep.Enabled {
			return a.snapshotLocked(), nil
		}
		delete(servers, item.Name)
	}
	if err := writeConfig(host.ConfigPath, host.Format, root); err != nil {
		return a.snapshotLocked(), err
	}
	if enabled {
		item.Deployments[hostID] = deployment{Enabled: true, Mode: "native", Target: host.ConfigPath}
	} else {
		delete(item.Deployments, hostID)
	}
	a.syncMCPConfigHashes(hostID, host.ConfigPath)
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) TestMCP(mcpID string) string {
	a.mu.Lock()
	defer a.mu.Unlock()
	item, ok := a.mcpByID(mcpID)
	if !ok {
		return "找不到 MCP"
	}
	if item.Type == "stdio" {
		if _, err := exec.LookPath(item.Command); err != nil {
			return fmt.Sprintf("找不到命令 %q", item.Command)
		}
		return "命令可用。Chu 不会在测试时启动服务。"
	}
	parsed, err := url.Parse(item.Endpoint)
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return "URL 格式无效"
	}
	request, err := http.NewRequest(http.MethodGet, item.Endpoint, nil)
	if err != nil {
		return "无法创建连接请求"
	}
	for key, value := range item.Headers {
		request.Header.Set(key, value)
	}
	if item.Secret != "" && request.Header.Get("Authorization") == "" {
		request.Header.Set("Authorization", "Bearer "+item.Secret)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	response, err := client.Do(request)
	if err != nil {
		return "连接失败: " + err.Error()
	}
	defer response.Body.Close()
	if response.StatusCode >= http.StatusOK && response.StatusCode < http.StatusBadRequest {
		return fmt.Sprintf("连接成功，HTTP %d", response.StatusCode)
	}
	return fmt.Sprintf("服务返回 HTTP %d", response.StatusCode)
}

func (a *App) AddAgent(input AgentInput) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	input.Name = strings.TrimSpace(input.Name)
	if input.Name == "" || strings.TrimSpace(input.Prompt) == "" {
		return a.snapshotLocked(), errors.New("Agent 名称和提示词不能为空")
	}
	for _, item := range a.state.Agents {
		if item.Name == input.Name {
			return a.snapshotLocked(), fmt.Errorf("Agent %q 已存在", input.Name)
		}
	}
	id := slug(input.Name)
	path := filepath.Join(a.root, "agents", id+".md")
	if err := atomicWrite(path, []byte(agentContent(input)), 0o600); err != nil {
		return a.snapshotLocked(), err
	}
	a.state.Agents = append(a.state.Agents, storedAgent{
		ID: id, Name: input.Name, Description: input.Description, Prompt: input.Prompt,
		Model: input.Model, Source: "chu", Deployments: map[string]deployment{},
	})
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ToggleAgent(agentID, hostID string, enabled bool) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	item, ok := a.agentByID(agentID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的自定义 Agent")
	}
	host, ok := a.hostByID(hostID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if item.Deployments == nil {
		item.Deployments = map[string]deployment{}
	}
	dep := item.Deployments[hostID]
	target := filepath.Join(host.AgentPath, item.ID+".md")
	if enabled {
		if dep.Enabled {
			return a.snapshotLocked(), nil
		}
		if fileExists(target) {
			return a.snapshotLocked(), fmt.Errorf("目标 Agent 已存在，请先处理冲突: %s", target)
		}
		if err := atomicWrite(target, []byte(agentContent(AgentInput{Name: item.Name, Description: item.Description, Prompt: item.Prompt, Model: item.Model})), 0o600); err != nil {
			return a.snapshotLocked(), err
		}
		item.Deployments[hostID] = deployment{Enabled: true, Mode: "copy", Target: target, LastHash: hashPath(target)}
	} else {
		if !dep.Enabled {
			return a.snapshotLocked(), nil
		}
		if hashPath(target) != dep.LastHash {
			return a.snapshotLocked(), errors.New("目标 Agent 已被修改，已保护用户改动")
		}
		if err := os.Remove(target); err != nil && !errors.Is(err, os.ErrNotExist) {
			return a.snapshotLocked(), err
		}
		delete(item.Deployments, hostID)
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) RestoreBackup(hostID string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	host, ok := a.hostByID(hostID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	backup := host.ConfigPath + ".chu.bak"
	if !fileExists(backup) {
		return a.snapshotLocked(), errors.New("没有可恢复的备份")
	}
	if fileExists(host.ConfigPath) {
		if err := copyFile(host.ConfigPath, host.ConfigPath+".chu.before-restore"); err != nil {
			return a.snapshotLocked(), err
		}
	}
	if err := copyFile(backup, host.ConfigPath); err != nil {
		return a.snapshotLocked(), err
	}
	backupHash := hashPath(host.ConfigPath)
	for i := range a.state.MCPs {
		if deployment, exists := a.state.MCPs[i].Deployments[hostID]; exists && deployment.Enabled {
			deployment.LastHash = backupHash
			a.state.MCPs[i].Deployments[hostID] = deployment
		}
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) UpdateHostPaths(hostID, configPath, skillPath, agentPath string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if _, ok := a.hostByID(hostID); !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if strings.TrimSpace(configPath) == "" || strings.TrimSpace(skillPath) == "" || strings.TrimSpace(agentPath) == "" {
		return a.snapshotLocked(), errors.New("宿主路径不能为空")
	}
	if a.state.Paths == nil {
		a.state.Paths = map[string]hostPath{}
	}
	a.state.Paths[hostID] = hostPath{Config: filepath.Clean(configPath), Skills: filepath.Clean(skillPath), Agents: filepath.Clean(agentPath)}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) snapshotLocked() Snapshot {
	specs := a.hostSpecs()
	hosts := make([]HostView, 0, len(specs))
	for _, spec := range specs {
		hosts = append(hosts, a.hostView(spec))
	}
	skills := make([]SkillView, 0, len(a.state.Skills))
	for _, item := range a.state.Skills {
		enabled, modes := deploymentViews(item.Deployments, specs)
		skills = append(skills, SkillView{ID: item.ID, Name: item.Name, Description: item.Description, Repository: item.Repository, Version: item.Version, Source: item.SourceDir, Managed: true, EnabledOn: enabled, ModeByHost: modes})
	}
	skills = append(skills, a.discoveredSkills(specs)...)
	sort.Slice(skills, func(i, j int) bool { return skills[i].Name < skills[j].Name })
	mcps := make([]MCPView, 0, len(a.state.MCPs))
	for _, item := range a.state.MCPs {
		enabled, _ := deploymentViews(item.Deployments, specs)
		mcps = append(mcps, MCPView{ID: item.ID, Name: item.Name, Description: item.Description, Type: item.Type, Endpoint: item.Endpoint, Command: item.Command, HasCredentials: item.Secret != "" || len(item.Env) > 0 || len(item.Headers) > 0, Managed: true, EnabledOn: enabled})
	}
	mcps = append(mcps, a.discoveredMCPs(specs)...)
	sort.Slice(mcps, func(i, j int) bool { return mcps[i].Name < mcps[j].Name })
	agents := make([]AgentView, 0, len(a.state.Agents))
	for _, item := range a.state.Agents {
		enabled, _ := deploymentViews(item.Deployments, specs)
		agents = append(agents, AgentView{ID: item.ID, Name: item.Name, Description: item.Description, Model: item.Model, Source: item.Source, Managed: true, EnabledOn: enabled})
	}
	return Snapshot{Root: a.root, Hosts: hosts, Skills: skills, MCPs: mcps, Agents: agents, LastScan: time.Now().Format(time.RFC3339)}
}

func (a *App) hostView(spec hostSpec) HostView {
	config := firstExisting(spec.config)
	if config == "" && len(spec.config) > 0 {
		config = spec.config[0]
	}
	skills := firstExisting(spec.skills)
	if skills == "" && len(spec.skills) > 0 {
		skills = spec.skills[0]
	}
	agents := firstExisting(spec.agents)
	if agents == "" && len(spec.agents) > 0 {
		agents = spec.agents[0]
	}
	installed := fileExists(config) || commandPresent(spec.id)
	status := "not-found"
	if installed {
		status = "ready"
	}
	format := "json"
	if strings.HasSuffix(config, ".toml") {
		format = "toml"
	}
	return HostView{ID: spec.id, Name: spec.name, Description: spec.description, Installed: installed, Status: status, ConfigPath: config, SkillPath: skills, AgentPath: agents, Format: format}
}

func (a *App) discoveredSkills(specs []hostSpec) []SkillView {
	known := map[string]bool{}
	for _, item := range a.state.Skills {
		known[item.Name] = true
	}
	var result []SkillView
	for _, spec := range specs {
		path := firstExisting(spec.skills)
		if path == "" {
			continue
		}
		entries, err := os.ReadDir(path)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if !entry.IsDir() || known[entry.Name()] || !fileExists(filepath.Join(path, entry.Name(), "SKILL.md")) {
				continue
			}
			id := "discovered-skill-" + spec.id + "-" + slug(entry.Name())
			result = append(result, SkillView{ID: id, Name: entry.Name(), Description: readSkillDescription(filepath.Join(path, entry.Name())), Source: filepath.Join(path, entry.Name()), Managed: false, EnabledOn: map[string]bool{spec.id: true}, ModeByHost: map[string]string{spec.id: "external"}})
		}
	}
	return result
}

func (a *App) discoveredMCPs(specs []hostSpec) []MCPView {
	known := map[string]bool{}
	for _, item := range a.state.MCPs {
		known[item.Name] = true
	}
	var result []MCPView
	for _, spec := range specs {
		config := firstExisting(spec.config)
		if config == "" {
			continue
		}
		root, err := readConfig(config, configFormat(config))
		if err != nil {
			continue
		}
		for name, raw := range ensureMap(root, mcpRootKey(spec.id)) {
			if known[name] {
				continue
			}
			server, _ := raw.(map[string]any)
			typeName := "stdio"
			endpoint := ""
			command := ""
			if value, ok := server["url"].(string); ok {
				endpoint = value
				typeName = "http"
			} else if value, ok := server["command"].(string); ok {
				command = value
			}
			result = append(result, MCPView{ID: "discovered-mcp-" + spec.id + "-" + slug(name), Name: name, Type: typeName, Endpoint: endpoint, Command: command, Managed: false, EnabledOn: map[string]bool{spec.id: true}})
		}
	}
	return result
}

func (a *App) loadState() error {
	data, err := os.ReadFile(filepath.Join(a.root, "state.json"))
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	return json.Unmarshal(data, &a.state)
}

func (a *App) saveStateLocked() error {
	a.state.Version = 1
	data, err := json.MarshalIndent(a.state, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(filepath.Join(a.root, "state.json"), append(data, '\n'), 0o600)
}

func (a *App) hostByID(id string) (HostView, bool) {
	for _, spec := range a.hostSpecs() {
		if spec.id == id {
			return a.hostView(spec), true
		}
	}
	return HostView{}, false
}

func (a *App) skillByID(id string) (*storedSkill, bool) {
	for i := range a.state.Skills {
		if a.state.Skills[i].ID == id {
			return &a.state.Skills[i], true
		}
	}
	return nil, false
}

func (a *App) mcpByID(id string) (*storedMCP, bool) {
	for i := range a.state.MCPs {
		if a.state.MCPs[i].ID == id {
			return &a.state.MCPs[i], true
		}
	}
	return nil, false
}

func (a *App) agentByID(id string) (*storedAgent, bool) {
	for i := range a.state.Agents {
		if a.state.Agents[i].ID == id {
			return &a.state.Agents[i], true
		}
	}
	return nil, false
}

func (a *App) validateMCPConfigHash(hostID, path string) error {
	current := hashPath(path)
	for _, item := range a.state.MCPs {
		if deployment, exists := item.Deployments[hostID]; exists && deployment.Enabled && deployment.LastHash != "" && deployment.LastHash != current {
			return errors.New("宿主配置在 Chu 外发生变化，已阻止写入")
		}
	}
	return nil
}

func (a *App) syncMCPConfigHashes(hostID, path string) {
	current := hashPath(path)
	for index := range a.state.MCPs {
		if deployment, exists := a.state.MCPs[index].Deployments[hostID]; exists && deployment.Enabled {
			deployment.LastHash = current
			a.state.MCPs[index].Deployments[hostID] = deployment
		}
	}
}

func (a *App) hostSpecs() []hostSpec {
	specs := defaultHostSpecs()
	for index := range specs {
		if override, ok := a.state.Paths[specs[index].id]; ok {
			specs[index].config = []string{override.Config}
			specs[index].skills = []string{override.Skills}
			specs[index].agents = []string{override.Agents}
		}
	}
	return specs
}

func defaultHostSpecs() []hostSpec {
	home, _ := os.UserHomeDir()
	configHome := os.Getenv("XDG_CONFIG_HOME")
	if configHome == "" {
		configHome = filepath.Join(home, ".config")
	}
	if runtime.GOOS == "windows" {
		if appData := os.Getenv("APPDATA"); appData != "" {
			configHome = appData
		}
	}
	opencodeConfig := []string{filepath.Join(configHome, "opencode", "opencode.json"), filepath.Join(configHome, "opencode.json")}
	opencodeSkills := []string{filepath.Join(configHome, "opencode", "skills"), filepath.Join(home, ".opencode", "skills")}
	opencodeAgents := []string{filepath.Join(configHome, "opencode", "agents"), filepath.Join(home, ".opencode", "agents")}
	return []hostSpec{
		{id: "opencode", name: "OpenCode", description: "本地开发 Agent", config: opencodeConfig, skills: opencodeSkills, agents: opencodeAgents},
		{id: "claude", name: "Claude Code", description: "Anthropic coding Agent", config: []string{filepath.Join(home, ".claude", "settings.json")}, skills: []string{filepath.Join(home, ".claude", "skills")}, agents: []string{filepath.Join(home, ".claude", "agents")}},
		{id: "codex", name: "Codex", description: "OpenAI coding Agent", config: []string{filepath.Join(home, ".codex", "config.toml")}, skills: []string{filepath.Join(home, ".codex", "skills")}, agents: []string{filepath.Join(home, ".codex", "agents")}},
	}
}

func deploymentViews(deployments map[string]deployment, specs []hostSpec) (map[string]bool, map[string]string) {
	enabled := map[string]bool{}
	modes := map[string]string{}
	for _, spec := range specs {
		if dep, ok := deployments[spec.id]; ok && dep.Enabled {
			enabled[spec.id] = true
			modes[spec.id] = dep.Mode
		} else {
			enabled[spec.id] = false
		}
	}
	return enabled, modes
}

func mcpRootKey(hostID string) string {
	if hostID == "claude" {
		return "mcpServers"
	}
	if hostID == "codex" {
		return "mcp_servers"
	}
	return "mcp"
}

func itemConfig(item storedMCP) map[string]any {
	result := map[string]any{}
	if item.Type == "stdio" {
		result["command"] = item.Command
		if len(item.Args) > 0 {
			result["args"] = item.Args
		}
	} else {
		result["url"] = item.Endpoint
		result["type"] = item.Type
	}
	if len(item.Env) > 0 {
		result["env"] = item.Env
	}
	if len(item.Headers) > 0 {
		result["headers"] = item.Headers
	}
	if item.Secret != "" {
		result["token"] = item.Secret
	}
	return result
}

func splitCommandLine(value string) (string, []string) {
	parts := strings.Fields(value)
	if len(parts) == 0 {
		return "", nil
	}
	return parts[0], parts[1:]
}

func agentContent(input AgentInput) string {
	var builder strings.Builder
	builder.WriteString("---\nname: ")
	builder.WriteString(input.Name)
	builder.WriteString("\ndescription: ")
	builder.WriteString(strings.ReplaceAll(input.Description, "\n", " "))
	if input.Model != "" {
		builder.WriteString("\nmodel: ")
		builder.WriteString(input.Model)
	}
	builder.WriteString("\n---\n\n")
	builder.WriteString(input.Prompt)
	builder.WriteString("\n")
	return builder.String()
}

func readConfig(path, format string) (map[string]any, error) {
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return map[string]any{}, nil
	}
	if err != nil {
		return nil, err
	}
	result := map[string]any{}
	if format == "toml" {
		if err := toml.Unmarshal(data, &result); err != nil {
			return nil, err
		}
		return result, nil
	}
	if len(strings.TrimSpace(string(data))) == 0 {
		return result, nil
	}
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, err
	}
	return result, nil
}

func writeConfig(path, format string, root map[string]any) error {
	var data []byte
	var err error
	if format == "toml" {
		data, err = toml.Marshal(root)
	} else {
		data, err = json.MarshalIndent(root, "", "  ")
		data = append(data, '\n')
	}
	if err != nil {
		return err
	}
	if fileExists(path) {
		if err := copyFile(path, path+".chu.bak"); err != nil {
			return fmt.Errorf("备份宿主配置失败: %w", err)
		}
	}
	return atomicWrite(path, data, 0o600)
}

func configFormat(path string) string {
	if strings.HasSuffix(path, ".toml") {
		return "toml"
	}
	return "json"
}

func ensureMap(root map[string]any, key string) map[string]any {
	if value, ok := root[key].(map[string]any); ok {
		return value
	}
	result := map[string]any{}
	root[key] = result
	return result
}

func firstExisting(paths []string) string {
	for _, path := range paths {
		if fileExists(path) {
			return path
		}
	}
	return ""
}

func commandPresent(name string) bool {
	_, err := exec.LookPath(name)
	return err == nil
}

func fileExists(path string) bool {
	_, err := os.Lstat(path)
	return err == nil
}

func isHTTPS(value string) bool {
	parsed, err := url.Parse(value)
	return err == nil && parsed.Scheme == "https" && parsed.Host != ""
}

func slug(value string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	var builder strings.Builder
	lastDash := false
	for _, char := range value {
		if (char >= 'a' && char <= 'z') || (char >= '0' && char <= '9') {
			builder.WriteRune(char)
			lastDash = false
		} else if !lastDash && builder.Len() > 0 {
			builder.WriteByte('-')
			lastDash = true
		}
	}
	return strings.Trim(builder.String(), "-")
}

func readSkillDescription(path string) string {
	data, err := os.ReadFile(filepath.Join(path, "SKILL.md"))
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "#") {
			return strings.TrimSpace(strings.TrimLeft(line, "#"))
		}
	}
	return ""
}

func copyDir(source, target string) error {
	return filepath.WalkDir(source, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		destination := filepath.Join(target, relative)
		if relative == "." {
			return os.MkdirAll(destination, 0o700)
		}
		if entry.Type()&os.ModeSymlink != 0 {
			link, readErr := os.Readlink(path)
			if readErr != nil {
				return readErr
			}
			if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
				return err
			}
			return os.Symlink(link, destination)
		}
		if entry.IsDir() {
			return os.MkdirAll(destination, 0o700)
		}
		return copyFile(path, destination)
	})
}

func copyFile(source, target string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()
	if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
		return err
	}
	output, err := os.OpenFile(target, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	if _, err := io.Copy(output, input); err != nil {
		_ = output.Close()
		return err
	}
	return output.Close()
}

func atomicWrite(path string, data []byte, mode os.FileMode) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	temporary, err := os.CreateTemp(filepath.Dir(path), ".chu-write-")
	if err != nil {
		return err
	}
	temporaryName := temporary.Name()
	defer os.Remove(temporaryName)
	if err := temporary.Chmod(mode); err != nil {
		_ = temporary.Close()
		return err
	}
	if _, err := temporary.Write(data); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Sync(); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(temporaryName, path)
}

func hashPath(path string) string {
	info, err := os.Lstat(path)
	if err != nil {
		return ""
	}
	hash := sha256.New()
	if info.IsDir() {
		_ = filepath.WalkDir(path, func(current string, entry os.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			relative, _ := filepath.Rel(path, current)
			if relative == "." {
				return nil
			}
			hash.Write([]byte(relative))
			if entry.Type()&os.ModeSymlink != 0 {
				link, _ := os.Readlink(current)
				hash.Write([]byte(link))
				return nil
			}
			if entry.IsDir() {
				return nil
			}
			data, readErr := os.ReadFile(current)
			if readErr == nil {
				hash.Write(data)
			}
			return nil
		})
	} else if data, err := os.ReadFile(path); err == nil {
		hash.Write(data)
	}
	return hex.EncodeToString(hash.Sum(nil))
}
