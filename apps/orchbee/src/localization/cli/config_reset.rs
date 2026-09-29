use crate::{cli::escape::quoted, localization::Locale};
use orchbee_local_site_runtime::{WindowsKeysResetOutcome, WindowsKeysResetReport};

impl Locale {
    pub(crate) fn cli_config_reset(self, report: &WindowsKeysResetReport) -> String {
        let path = report
            .path
            .as_deref()
            .and_then(std::path::Path::to_str)
            .map(quoted);
        let mut text = match (&report.outcome, self) {
            (WindowsKeysResetOutcome::Missing, Self::EnUs) => {
                "No configuration file; nothing changed.\n".to_owned()
            }
            (WindowsKeysResetOutcome::Missing, Self::ZhCn) => {
                "配置文件不存在，未做更改。\n".to_owned()
            }
            (WindowsKeysResetOutcome::NoKeys, Self::EnUs) => {
                "No custom keys; nothing changed.\n".to_owned()
            }
            (WindowsKeysResetOutcome::NoKeys, Self::ZhCn) => {
                "没有自定义键位，未做更改。\n".to_owned()
            }
            (WindowsKeysResetOutcome::Reset { .. }, Self::EnUs) => {
                "Custom keys removed. Running Clients were not reloaded.\n".to_owned()
            }
            (WindowsKeysResetOutcome::Reset { .. }, Self::ZhCn) => {
                "已移除自定义键位；未重载运行中的 Client。\n".to_owned()
            }
            (WindowsKeysResetOutcome::Failed { error, .. }, Self::EnUs) => format!(
                "Keybinding reset failed ({}: {}).\n",
                error.field, error.reason
            ),
            (WindowsKeysResetOutcome::Failed { error, .. }, Self::ZhCn) => {
                format!("快捷键恢复失败（{}: {}）。\n", error.field, error.reason)
            }
        };
        if let Some(path) = path {
            text.push_str(&match self {
                Self::EnUs => format!("Configuration: {path}\n"),
                Self::ZhCn => format!("配置：{path}\n"),
            });
        }
        let (backup, recovery) = match &report.outcome {
            WindowsKeysResetOutcome::Reset { backup_path } => (Some(backup_path), false),
            WindowsKeysResetOutcome::Failed {
                backup_path,
                recovery_required,
                ..
            } => (backup_path.as_ref(), *recovery_required),
            _ => (None, false),
        };
        if let Some(backup) = backup.and_then(|path| path.to_str()).map(quoted) {
            text.push_str(&match self {
                Self::EnUs => format!("Retained backup: {backup}\nYou own this file; retain it for recovery, then remove it when no longer needed.\n"),
                Self::ZhCn => format!("保留的备份：{backup}\n此文件已交付给你；恢复需要结束后可自行删除。\n"),
            });
        }
        if recovery {
            text.push_str(match self {
                Self::EnUs => "The original could not be restored. Restore the reported backup before using this configuration.\n",
                Self::ZhCn => "原配置恢复失败。请先从上述备份恢复，再使用此配置。\n",
            });
        }
        text
    }
}
