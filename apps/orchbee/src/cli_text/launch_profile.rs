//! Localized launch-profile failures and command help; does not launch processes.

use super::Locale;
use super::failure_target;
use crate::cli::Failure;

pub(super) const HELP_LAUNCH_PROFILE_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile list [--format text|json]",
        "List every Launch Profile in the running local session's catalog: profile\nID, name, and the stored exec/args launch. '--format json' emits the\norchbee.launch-profile.list envelope with the catalog revision.",
    );

pub(super) const HELP_LAUNCH_PROFILE_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile list [--format text|json]",
        "列出运行中本地会话目录里的所有启动配置：profile ID、名称及存储的\nexec/args 启动。'--format json' 输出携带目录修订的\norchbee.launch-profile.list 封套。",
    );

pub(super) const HELP_LAUNCH_PROFILE_GET_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile get <selector> [--format text|json]",
        "Show one Launch Profile resolved by the Server: an exact profile ID\nfirst, then a name whose match cardinality must be exactly one — an\nunmatched or ambiguous selector fails, never guesses. '--format json'\nemits the orchbee.launch-profile.get envelope.",
    );

pub(super) const HELP_LAUNCH_PROFILE_GET_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile get <selector> [--format text|json]",
        "显示一个由 Server 解析的启动配置：先按精确 profile ID，再按匹配数必须恰好为一的名称——未命中或歧义的 selector 会失败而非猜测。\n'--format json' 输出 orchbee.launch-profile.get 封套。",
    );

pub(super) const HELP_LAUNCH_PROFILE_CREATE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]",
        "Store one named Launch Profile in the running session's catalog. '--name'\nis required (1–512 UTF-8 bytes; uniqueness is not required). '--exec'\nselects the launched program by absolute path and each '--arg' appends\none verbatim argument. The result carries the committed revision, the\ncatalog revision, the minted profile ID, and the stored name.",
    );

pub(super) const HELP_LAUNCH_PROFILE_CREATE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]",
        "在运行中会话的目录里存储一个命名启动配置。'--name' 必填（1–512 个\nUTF-8 字节；不要求唯一）。'--exec' 以绝对路径选择启动程序，每个\n'--arg' 逐字追加一个参数。结果携带已提交修订、目录修订、新铸造的\nprofile ID 及存储的名称。",
    );

pub(super) const HELP_LAUNCH_PROFILE_DELETE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile delete <selector> [--format text|json]",
        "Delete one Launch Profile resolved by the Server under the same selector\nrule as 'launch-profile get': exact profile ID first, then a name\nmatching exactly one entry. The result carries the committed revision,\nthe catalog revision, and the deleted profile ID.",
    );

pub(super) const HELP_LAUNCH_PROFILE_DELETE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee launch-profile delete <selector> [--format text|json]",
        "删除一个由 Server 按与 'launch-profile get' 相同 selector 规则解析的启动配置：先精确 profile ID，再按恰好匹配一个条目的名称。结果携带已提交修订、目录修订与被删除的 profile ID。",
    );

impl super::CliText {
    /// `launch-profile` failure diagnostics, keyed by the details the typed
    /// catalog outcome mapping produced. Every line states what the catalog
    /// resolution observed — the CLI never resolved the selector itself.
    pub(super) fn cli_launch_profile_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        let detail = |name: &str| {
            failure
                .details
                .as_ref()
                .and_then(|details| details.get(name))
                .and_then(|value| value.as_str())
                .unwrap_or("")
        };
        match failure.kind {
            FailureKind::NotFound => {
                let (_resource, target) = failure_target(failure);
                let escaped = crate::cli::escape::quoted(&target);
                match self.locale {
                    Locale::EnUs => {
                        format!("No launch profile matches {escaped}.\n")
                    }
                    Locale::ZhCn => format!("没有与 {escaped} 匹配的启动配置。\n"),
                }
            }
            FailureKind::Ambiguous => {
                let (_resource, target) = failure_target(failure);
                let escaped = crate::cli::escape::quoted(&target);
                match self.locale {
                    Locale::EnUs => {
                        format!(
                            "More than one launch profile matches {escaped}; use its exact profile ID.\n"
                        )
                    }
                    Locale::ZhCn => {
                        format!("多个启动配置与 {escaped} 匹配；请使用精确的 profile ID。\n")
                    }
                }
            }
            FailureKind::Denied => match self.locale {
                Locale::EnUs => "The local session denied the launch profile request.\n".to_owned(),
                Locale::ZhCn => "本地会话拒绝了该启动配置请求。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => match self.locale {
                Locale::EnUs => {
                    "The local session could not answer the launch profile request right now.\n"
                        .to_owned()
                }
                Locale::ZhCn => "本地会话当前无法应答该启动配置请求。\n".to_owned(),
            },
            FailureKind::Timeout => match self.locale {
                Locale::EnUs => {
                    "The launch profile request did not complete within the time limit.\n"
                        .to_owned()
                }
                Locale::ZhCn => "启动配置请求未在时限内完成。\n".to_owned(),
            },
            FailureKind::TransportLoss => match self.locale {
                Locale::EnUs => {
                    "The connection to the local session was lost during the launch profile request.\n"
                        .to_owned()
                }
                Locale::ZhCn => "与本地会话的连接在启动配置请求期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => {
                let reason = detail("reason");
                match (self.locale, reason) {
                    (Locale::EnUs, "correlation-mismatch") => {
                        "The local session answered a different launch profile request.\n".to_owned()
                    }
                    (Locale::ZhCn, "correlation-mismatch") => {
                        "本地会话应答了另一个启动配置请求。\n".to_owned()
                    }
                    (Locale::EnUs, _) => {
                        "The local session answered the launch profile request with an invalid protocol response.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, _) => {
                        "本地会话对启动配置请求返回了无效的协议响应。\n".to_owned()
                    }
                }
            }
            _ => match self.locale {
                Locale::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Locale::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }
}
