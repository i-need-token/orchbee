use super::Locale;
use crate::terminal_session::{NativeLocationLabelStatus, NativeLocationLabelTarget};

impl Locale {
    pub(crate) const fn location_label_status(
        self,
        target: NativeLocationLabelTarget,
        status: NativeLocationLabelStatus,
    ) -> &'static str {
        use NativeLocationLabelStatus as Status;
        use NativeLocationLabelTarget as Target;
        match (self, target, status) {
            (Self::EnUs, Target::Project, Status::Unavailable) => {
                "No current Project to rename · no command was sent"
            }
            (Self::ZhCn, Target::Project, Status::Unavailable) => {
                "当前没有可重命名的项目 · 未发送命令"
            }
            (Self::EnUs, Target::Workspace, Status::Unavailable) => {
                "No current Workspace to rename · no command was sent"
            }
            (Self::ZhCn, Target::Workspace, Status::Unavailable) => {
                "当前没有可重命名的工作区 · 未发送命令"
            }
            (Self::EnUs, Target::Project, Status::NotApplied) => "Project rename not applied",
            (Self::ZhCn, Target::Project, Status::NotApplied) => "项目重命名未应用",
            (Self::EnUs, Target::Workspace, Status::NotApplied) => "Workspace rename not applied",
            (Self::ZhCn, Target::Workspace, Status::NotApplied) => "工作区重命名未应用",
            (Self::EnUs, Target::Project, Status::Unchanged) => "Project label unchanged",
            (Self::ZhCn, Target::Project, Status::Unchanged) => "项目名称未更改",
            (Self::EnUs, Target::Workspace, Status::Unchanged) => "Workspace label unchanged",
            (Self::ZhCn, Target::Workspace, Status::Unchanged) => "工作区名称未更改",
            (Self::EnUs, Target::Project, Status::Renamed) => "Project renamed",
            (Self::ZhCn, Target::Project, Status::Renamed) => "项目已重命名",
            (Self::EnUs, Target::Workspace, Status::Renamed) => "Workspace renamed",
            (Self::ZhCn, Target::Workspace, Status::Renamed) => "工作区已重命名",
            (Self::EnUs, Target::Project, Status::CommittedNeedRefresh) => {
                "Project rename result retained · Ctrl+B w to recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, Target::Project, Status::CommittedNeedRefresh) => {
                "项目重命名结果已保留 · 按 Ctrl+B w 恢复快照；请勿重复提交"
            }
            (Self::EnUs, Target::Workspace, Status::CommittedNeedRefresh) => {
                "Workspace rename result retained · Ctrl+B w to recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, Target::Workspace, Status::CommittedNeedRefresh) => {
                "工作区重命名结果已保留 · 按 Ctrl+B w 恢复快照；请勿重复提交"
            }
            (Self::EnUs, Target::Project, Status::OutcomeUnknown) => {
                "Project rename outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, Target::Project, Status::OutcomeUnknown) => {
                "项目重命名结果未知 · 连接已退役；请勿重复变更"
            }
            (Self::EnUs, Target::Workspace, Status::OutcomeUnknown) => {
                "Workspace rename outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, Target::Workspace, Status::OutcomeUnknown) => {
                "工作区重命名结果未知 · 连接已退役；请勿重复变更"
            }
        }
    }

    pub(super) const fn location_rename_committed_fact(
        self,
        target: NativeLocationLabelTarget,
    ) -> &'static str {
        match (self, target) {
            (Self::EnUs, NativeLocationLabelTarget::Project) => {
                "Project rename result retained · do not resubmit"
            }
            (Self::ZhCn, NativeLocationLabelTarget::Project) => {
                "项目重命名结果已保留 · 请勿重复提交"
            }
            (Self::EnUs, NativeLocationLabelTarget::Workspace) => {
                "Workspace rename result retained · do not resubmit"
            }
            (Self::ZhCn, NativeLocationLabelTarget::Workspace) => {
                "工作区重命名结果已保留 · 请勿重复提交"
            }
        }
    }

    pub(crate) const fn location_rename_title(self, project: bool) -> &'static str {
        match (self, project) {
            (Self::EnUs, true) => "Rename Project",
            (Self::ZhCn, true) => "重命名项目",
            (Self::EnUs, false) => "Rename Workspace",
            (Self::ZhCn, false) => "重命名工作区",
        }
    }

    pub(crate) const fn location_rename_actions(self, project: bool) -> &'static str {
        match (self, project) {
            (Self::EnUs, true) => {
                "Enter · rename Project    Esc · cancel    Backspace · delete    Ctrl+U · clear"
            }
            (Self::ZhCn, true) => {
                "Enter · 重命名项目    Esc · 取消    Backspace · 删除    Ctrl+U · 清空"
            }
            (Self::EnUs, false) => {
                "Enter · rename Workspace    Esc · cancel    Backspace · delete    Ctrl+U · clear"
            }
            (Self::ZhCn, false) => {
                "Enter · 重命名工作区    Esc · 取消    Backspace · 删除    Ctrl+U · 清空"
            }
        }
    }

    pub(crate) const fn location_rename_hint(self, project: bool) -> &'static str {
        match (self, project) {
            (Self::EnUs, true) => {
                "Use 1–512 characters. The shared Project label changes for all Clients."
            }
            (Self::ZhCn, true) => "请输入 1–512 个字符。共享项目的名称会对所有 Client 更改。",
            (Self::EnUs, false) => {
                "Use 1–512 characters. The shared Workspace label changes for all Clients."
            }
            (Self::ZhCn, false) => "请输入 1–512 个字符。共享工作区的名称会对所有 Client 更改。",
        }
    }

    pub(crate) const fn location_rename_resize_hint(self, project: bool) -> &'static str {
        match (self, project) {
            (Self::EnUs, true) => {
                "Enlarge the window to see the complete Project rename, or press Esc to cancel."
            }
            (Self::ZhCn, true) => "请扩大窗口以查看完整项目重命名，或按 Esc 取消。",
            (Self::EnUs, false) => {
                "Enlarge the window to see the complete Workspace rename, or press Esc to cancel."
            }
            (Self::ZhCn, false) => "请扩大窗口以查看完整工作区重命名，或按 Esc 取消。",
        }
    }
}
