//! 正常关闭先安全终结后台工作，任何事件循环返回路径都回收线程。
use crate::controller::Controller;
use anyhow::Result;
use slint::ComponentHandle;
impl Controller {
    pub(crate) fn request_shutdown(&self) {
        if self.closing.replace(true) {
            return;
        }
        self.worker.borrow_mut().begin_shutdown();
        self.update_shutdown_ui();
    }
    pub(crate) fn update_shutdown_ui(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        ui.set_closing(true);
        ui.set_busy(true);
        ui.set_task_can_cancel(false);
        ui.set_task_phase(2);
        ui.set_task_label("正在安全收尾，完成提交或回滚后自动退出…".into());
        ui.set_dialog_can_cancel(false);
        ui.set_dialog_can_submit(false);
        ui.set_dialog_busy(true);
        if ui.get_dialog_open() {
            ui.set_dialog_error("正在安全退出，请等待后台提交或清理完成。".into());
        }
    }
    pub(crate) fn poll_shutdown(&self) {
        if self.closed.get() {
            return;
        }
        self.update_shutdown_ui();
        if !self.worker.borrow().is_finished() {
            return;
        }
        if let Err(error) = self.worker.borrow_mut().join() {
            self.notice(&error.to_string(), true);
        }
        self.closed.set(true);
        self.timer.stop();
        if let Some(ui) = self.window.upgrade() {
            let _ = ui.hide();
        }
        let _ = slint::quit_event_loop();
    }
    /// 事件循环已停止时使用的同步兜底，不得在普通 Slint 交互回调中调用。
    pub fn shutdown(&self) -> Result<()> {
        self.closing.set(true);
        self.timer.stop();
        let result = self.worker.borrow_mut().join();
        self.closed.set(true);
        result
    }
}
