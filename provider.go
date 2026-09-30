package main

import (
	"crypto/rand"
	"crypto/tls"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"syscall"
	"time"
)

type ProviderInput struct {
	Name      string `json:"name"`
	APIKey    string `json:"apiKey"`
	EnvAPIKey string `json:"envApiKey"`
	BaseURL   string `json:"baseUrl"`
}

type ProviderView struct {
	ID              string   `json:"id"`
	Name            string   `json:"name"`
	EnvAPIKey       string   `json:"envApiKey"`
	BaseURL         string   `json:"baseUrl"`
	Models          []string `json:"models"`
	ModelsFetchedAt string   `json:"modelsFetchedAt"`
	ModelsError     string   `json:"modelsError"`
}

type providerRequest struct {
	provider ProviderView
	key      string
	err      error
}

var providerEnvName = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)

func (a *App) providerViewsLocked() []ProviderView {
	views := make([]ProviderView, len(a.state.Providers))
	for i, p := range a.state.Providers {
		p.Models = append([]string{}, p.Models...)
		views[i] = p
	}
	return views
}

func (a *App) providerIndex(id string) int {
	for i, p := range a.state.Providers {
		if p.ID == id {
			return i
		}
	}
	return -1
}

func sameProviderEnv(a, b string) bool {
	if runtime.GOOS == "windows" {
		return strings.EqualFold(a, b)
	}
	return a == b
}

func (a *App) validateProvider(id string, input *ProviderInput) error {
	input.Name = strings.TrimSpace(input.Name)
	input.BaseURL = strings.TrimRight(strings.TrimSpace(input.BaseURL), "/")
	if input.Name == "" {
		return errors.New("供应商名称不能为空")
	}
	if input.EnvAPIKey != "" && !providerEnvName.MatchString(input.EnvAPIKey) {
		return errors.New("API Key 环境变量名无效")
	}
	if input.APIKey != "" && input.EnvAPIKey == "" {
		return errors.New("填写 API Key 时必须指定环境变量名")
	}
	u, err := url.Parse(input.BaseURL)
	if err != nil || u.Hostname() == "" || (u.Scheme != "http" && u.Scheme != "https") || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
		return errors.New("Base URL 必须是有效的 HTTP/HTTPS 地址，不能包含凭证、查询或片段")
	}
	for _, p := range a.state.Providers {
		if p.ID == id {
			continue
		}
		if strings.EqualFold(p.Name, input.Name) {
			return errors.New("供应商名称已存在")
		}
		if input.EnvAPIKey != "" && sameProviderEnv(p.EnvAPIKey, input.EnvAPIKey) {
			return errors.New("API Key 环境变量已被其他供应商引用")
		}
	}
	return nil
}

func (a *App) CreateProvider(input ProviderInput) (Snapshot, error) {
	return a.saveProvider("", input)
}

func (a *App) UpdateProvider(id string, input ProviderInput) (Snapshot, error) {
	if id == "" {
		a.mu.Lock()
		defer a.mu.Unlock()
		return a.snapshotLocked(), errors.New("未知的供应商")
	}
	return a.saveProvider(id, input)
}

func (a *App) saveProvider(id string, input ProviderInput) (Snapshot, error) {
	a.mu.Lock()
	i := a.providerIndex(id)
	if id != "" && i < 0 {
		defer a.mu.Unlock()
		return a.snapshotLocked(), errors.New("未知的供应商")
	}
	if err := a.validateProvider(id, &input); err != nil {
		defer a.mu.Unlock()
		return a.snapshotLocked(), err
	}
	p := ProviderView{ID: id, Models: []string{}}
	changed := i < 0
	if i >= 0 {
		p = a.state.Providers[i]
		changed = p.BaseURL != input.BaseURL || p.EnvAPIKey != input.EnvAPIKey || input.APIKey != ""
	} else {
		var random [16]byte
		if _, err := rand.Read(random[:]); err != nil {
			defer a.mu.Unlock()
			return a.snapshotLocked(), errors.New("生成供应商 ID 失败")
		}
		p.ID = hex.EncodeToString(random[:])
	}
	p.Name, p.BaseURL, p.EnvAPIKey = input.Name, input.BaseURL, input.EnvAPIKey
	if changed {
		p.Models, p.ModelsFetchedAt, p.ModelsError = []string{}, "", ""
	}
	providers := a.providerViewsLocked()
	if i < 0 {
		providers = append(providers, p)
	} else {
		providers[i] = p
	}
	if err := a.commitProvidersLocked(providers, input.EnvAPIKey, input.APIKey); err != nil {
		defer a.mu.Unlock()
		return a.snapshotLocked(), err
	}
	if !changed {
		defer a.mu.Unlock()
		return a.snapshotLocked(), nil
	}
	request := a.startProviderRequestLocked(p)
	a.mu.Unlock()
	return a.finishProviderRequest(request)
}

func (a *App) DeleteProvider(id string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	i := a.providerIndex(id)
	if i < 0 {
		return a.snapshotLocked(), errors.New("未知的供应商")
	}
	providers := a.providerViewsLocked()
	providers = append(providers[:i], providers[i+1:]...)
	if err := a.commitProvidersLocked(providers, "", ""); err != nil {
		return a.snapshotLocked(), err
	}
	delete(a.providerRequests, id)
	return a.snapshotLocked(), nil
}

func (a *App) RefreshProviderModels(id string) (Snapshot, error) {
	a.mu.Lock()
	i := a.providerIndex(id)
	if i < 0 {
		defer a.mu.Unlock()
		return a.snapshotLocked(), errors.New("未知的供应商")
	}
	request := a.startProviderRequestLocked(a.state.Providers[i])
	a.mu.Unlock()
	return a.finishProviderRequest(request)
}

func (a *App) startProviderRequestLocked(p ProviderView) *providerRequest {
	r := &providerRequest{provider: p}
	r.key, r.err = providerKey(a.root, p.EnvAPIKey)
	if a.providerRequests == nil {
		a.providerRequests = make(map[string]*providerRequest)
	}
	a.providerRequests[p.ID] = r
	return r
}

func (a *App) finishProviderRequest(r *providerRequest) (Snapshot, error) {
	models, err := []string{}, r.err
	if err == nil {
		models, err = fetchProviderModels(r.provider.BaseURL, r.key)
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	i := a.providerIndex(r.provider.ID)
	if i < 0 || a.providerRequests[r.provider.ID] != r {
		return a.snapshotLocked(), nil
	}
	delete(a.providerRequests, r.provider.ID)
	providers := a.providerViewsLocked()
	p := &providers[i]
	if err != nil {
		p.ModelsError = err.Error()
	} else {
		p.Models, p.ModelsFetchedAt, p.ModelsError = models, time.Now().UTC().Format(time.RFC3339Nano), ""
	}
	if err := a.commitProvidersLocked(providers, "", ""); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) commitProvidersLocked(providers []ProviderView, envName, key string) error {
	next := a.state
	next.Version, next.Providers = 1, providers
	data, err := json.MarshalIndent(next, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	if key != "" {
		err = saveProviderCredential(a.root, envName, key, data)
	} else {
		err = atomicWrite(filepath.Join(a.root, "state.json"), data, 0o600)
	}
	if err == nil {
		a.state = next
	}
	return err
}

func fetchProviderModels(baseURL, key string) ([]string, error) {
	req, err := http.NewRequest(http.MethodGet, baseURL+"/models", nil)
	if err != nil {
		return nil, errors.New("无法创建模型请求")
	}
	if key != "" {
		req.Header.Set("Authorization", "Bearer "+key)
	}
	client := &http.Client{Timeout: 15 * time.Second, CheckRedirect: func(next *http.Request, via []*http.Request) error {
		// Reject all redirects so credentials never leave the configured endpoint.
		return http.ErrUseLastResponse
	}}
	response, err := client.Do(req)
	if err != nil {
		var networkError net.Error
		var dnsError *net.DNSError
		var certificateError *tls.CertificateVerificationError
		switch {
		case errors.As(err, &networkError) && networkError.Timeout():
			return nil, errors.New("模型请求超时（15 秒）")
		case errors.As(err, &dnsError):
			return nil, errors.New("模型请求失败：无法解析服务器域名")
		case errors.As(err, &certificateError):
			return nil, errors.New("模型请求失败：TLS 证书验证失败")
		case errors.Is(err, syscall.ECONNREFUSED):
			return nil, errors.New("模型请求失败：服务器拒绝连接")
		default:
			return nil, errors.New("模型请求失败：连接异常或请求无法发送")
		}
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, fmt.Errorf("模型请求返回 HTTP %d", response.StatusCode)
	}
	const limit = 4 << 20
	body, err := io.ReadAll(io.LimitReader(response.Body, limit+1))
	if err != nil {
		return nil, errors.New("读取模型响应失败")
	}
	if len(body) > limit {
		return nil, errors.New("模型响应超过 4 MiB 限制")
	}
	var envelope struct {
		Data json.RawMessage `json:"data"`
	}
	if json.Unmarshal(body, &envelope) != nil {
		return nil, errors.New("模型响应不是有效 JSON")
	}
	var entries []struct {
		ID string `json:"id"`
	}
	if len(envelope.Data) == 0 || string(envelope.Data) == "null" || json.Unmarshal(envelope.Data, &entries) != nil {
		return nil, errors.New("模型响应必须包含 data 数组")
	}
	models := make([]string, 0, len(entries))
	for _, entry := range entries {
		if strings.TrimSpace(entry.ID) == "" {
			return nil, errors.New("模型响应包含缺失或空的模型 ID")
		}
		models = append(models, entry.ID)
	}
	return models, nil
}
