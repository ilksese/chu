use crate::{App, credentials, fs::*, models::*};
use anyhow::{Context, Result, bail};
use std::{io::Read, time::Duration};
fn valid_env(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
pub fn fetch_models(base: &str, key: &str) -> Result<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = client.get(format!("{base}/models"));
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request.send().map_err(|e| {
        if e.is_timeout() {
            anyhow::anyhow!("模型请求超时（15 秒）")
        } else {
            anyhow::anyhow!("模型请求失败：连接异常或请求无法发送")
        }
    })?;
    if !response.status().is_success() {
        bail!("模型请求返回 HTTP {}", response.status().as_u16());
    }
    const LIMIT: usize = 4 << 20;
    let mut body = Vec::new();
    response
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut body)
        .context("读取模型响应失败")?;
    if body.len() > LIMIT {
        bail!("模型响应超过 4 MiB 限制");
    }
    let value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| anyhow::anyhow!("模型响应不是有效 JSON"))?;
    let data = value["data"]
        .as_array()
        .context("模型响应必须包含 data 数组")?;
    data.iter()
        .map(|entry| {
            entry["id"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
                .context("模型响应包含缺失或空的模型 ID")
        })
        .collect()
}
impl App {
    fn provider_index(&self, id: &str) -> Result<usize> {
        self.state
            .providers
            .iter()
            .position(|p| p.id == id)
            .context("未知的供应商")
    }
    fn validate_provider(&self, id: &str, input: &mut ProviderInput) -> Result<()> {
        input.name = input.name.trim().into();
        input.base_url = input.base_url.trim().trim_end_matches('/').into();
        if input.name.is_empty() {
            bail!("供应商名称不能为空");
        }
        if !input.env_api_key.is_empty() && !valid_env(&input.env_api_key) {
            bail!("API Key 环境变量名无效");
        }
        if !input.api_key.is_empty() && input.env_api_key.is_empty() {
            bail!("填写 API Key 时必须指定环境变量名");
        }
        let u =
            url::Url::parse(&input.base_url).context("Base URL 必须是有效的 HTTP/HTTPS 地址")?;
        if !["http", "https"].contains(&u.scheme())
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
        {
            bail!("Base URL 必须是有效的 HTTP/HTTPS 地址，不能包含凭证、查询或片段");
        }
        for p in self.state.providers.iter().filter(|p| p.id != id) {
            if p.name.to_lowercase() == input.name.to_lowercase() {
                bail!("供应商名称已存在");
            }
            if !input.env_api_key.is_empty()
                && credentials::same_env(&p.env_api_key, &input.env_api_key)
            {
                bail!("API Key 环境变量已被其他供应商引用");
            }
        }
        Ok(())
    }
    fn commit_providers(&mut self, providers: Vec<Provider>, env: &str, key: &str) -> Result<()> {
        let mut next = self.state.clone();
        next.version = 1;
        next.providers = providers;
        let mut data = serde_json::to_vec_pretty(&next)?;
        data.push(b'\n');
        if key.is_empty() {
            atomic_write(&self.root.join("state.json"), &data)?;
        } else {
            credentials::save(&self.root, env, key, &data)?;
        }
        self.state = next;
        Ok(())
    }
    pub fn save_provider(&mut self, id: &str, mut input: ProviderInput) -> Result<()> {
        let index = if id.is_empty() {
            None
        } else {
            Some(self.provider_index(id)?)
        };
        self.validate_provider(id, &mut input)?;
        let old = index.map(|i| self.state.providers[i].clone());
        let changed = old
            .as_ref()
            .is_none_or(|p| p.base_url != input.base_url || p.env_api_key != input.env_api_key)
            || !input.api_key.is_empty();
        let mut provider = old.unwrap_or_else(|| Provider {
            id: format!("provider-{}", uuid::Uuid::new_v4().simple()),
            ..Provider::default()
        });
        provider.name = input.name;
        provider.base_url = input.base_url;
        provider.env_api_key = input.env_api_key;
        let mut providers = self.state.providers.clone();
        if let Some(i) = index {
            providers[i] = provider.clone();
        } else {
            providers.push(provider.clone());
        }
        self.commit_providers(providers, &provider.env_api_key, &input.api_key)?;
        if changed {
            self.refresh_provider_models(&provider.id)?;
        }
        Ok(())
    }
    pub fn delete_provider(&mut self, id: &str) -> Result<()> {
        let i = self.provider_index(id)?;
        let mut providers = self.state.providers.clone();
        providers.remove(i);
        self.commit_providers(providers, "", "")
    }
    /// Mutations are serialized on the worker. Exclusive &mut borrowing makes stale overwrites impossible.
    pub fn refresh_provider_models(&mut self, id: &str) -> Result<()> {
        let i = self.provider_index(id)?;
        let provider = self.state.providers[i].clone();
        let result = credentials::key(&self.root, &provider.env_api_key)
            .and_then(|key| fetch_models(&provider.base_url, &key));
        let mut providers = self.state.providers.clone();
        let p = &mut providers[i];
        match result {
            Ok(models) => {
                p.models = models;
                p.models_fetched_at = now();
                p.models_error.clear();
            }
            Err(e) => p.models_error = e.to_string(),
        }
        self.commit_providers(providers, "", "")
    }
}
