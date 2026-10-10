//! 页面工作位置只存在于当前会话，不写入中央资源状态。
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct PagePosition {
    pub query: String,
    pub remembered: String,
    pub list_scroll: f32,
}
#[derive(Clone, Default)]
pub struct DetailPosition {
    pub scroll: f32,
    pub source: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub enum DetailLoad {
    #[default]
    NotLoaded,
    Loading,
    Ready,
    Failed(String),
}
#[derive(Default)]
pub struct Session {
    pub pages: BTreeMap<i32, PagePosition>,
    pub details: BTreeMap<String, DetailPosition>,
    pub loads: BTreeMap<String, DetailLoad>,
    pub selected_revision: u64,
    pub settings_host: String,
}
impl Session {
    pub fn remember_selection(&mut self, page: i32, id: &str) {
        self.pages.entry(page).or_default().remembered = id.into();
        self.selected_revision += 1;
    }
    pub fn invalidate_details(&mut self) {
        self.loads.clear();
    }
}
