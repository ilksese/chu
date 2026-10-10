//! 导入选择使用候选指纹身份；筛选仅改变可见投影。
use crate::models::{ImportCandidate, ImportScan};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
#[path = "import_state_tests.rs"]
mod tests;
#[derive(Default)]
pub(crate) struct ImportState {
    pub candidates: Vec<ImportCandidate>,
    pub selected: BTreeSet<String>,
    pub query: String,
    pub host_filter: usize,
    pub hosts: Vec<(String, String)>,
}
impl ImportState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn merge(&mut self, scan: ImportScan) -> (Vec<String>, usize) {
        let previous_host = self
            .host_filter
            .checked_sub(1)
            .and_then(|i| self.hosts.get(i))
            .map(|(id, _)| id.clone());
        let old = self.selected.len();
        self.selected
            .retain(|id| scan.candidates.iter().any(|c| &c.id == id && c.available));
        self.candidates = scan.candidates;
        let mut hosts = BTreeMap::new();
        for c in &self.candidates {
            hosts.insert(c.host_id.clone(), c.host_name.clone());
        }
        self.hosts = hosts.into_iter().collect();
        self.host_filter = previous_host
            .and_then(|id| {
                self.hosts
                    .iter()
                    .position(|(host, _)| host == &id)
                    .map(|i| i + 1)
            })
            .unwrap_or(0);
        (scan.warnings, old.saturating_sub(self.selected.len()))
    }
    pub fn visible(&self) -> Vec<&ImportCandidate> {
        let query = self.query.trim().to_lowercase();
        let host = self
            .host_filter
            .checked_sub(1)
            .and_then(|i| self.hosts.get(i))
            .map(|(id, _)| id.as_str());
        self.candidates
            .iter()
            .filter(|c| {
                host.is_none_or(|h| h == c.host_id)
                    && (query.is_empty()
                        || format!("{} {} {} {}", c.name, c.description, c.source, c.host_name)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect()
    }
    pub fn selected_candidates(&self) -> Vec<&ImportCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.available && self.selected.contains(&c.id))
            .collect()
    }
    pub fn conflicts(&self) -> String {
        let mut names = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for c in self.selected_candidates() {
            if !names.insert(c.name.clone())
                || !ids.insert(crate::fs::identifier(
                    &c.name,
                    if c.r#type == "skill" { "skill" } else { "mcp" },
                ))
            {
                return "已选来源中存在同名或标识冲突。请取消不需要的来源，再导入；已有选择不会自动替换。".into();
            }
        }
        String::new()
    }
}
