//! `doctor` copy: check labels, status words, verdicts and the one fact or reason each line shows.
use super::Locale;

impl super::CliText {
    /// The label of one check, by its JSON key; the Session check is the last key.
    pub(crate) fn cli_doctor_label(self, key: &str) -> &'static str {
        let (en, zh) = match key {
            "installation" => ("Installation", "安装"),
            "settings" => ("Settings", "设置"),
            "records" => ("Saved data", "已保存数据"),
            "runtime" => ("Runtime folder", "运行时目录"),
            _ => ("Session", "会话"),
        };
        self.pick(en, zh)
    }

    /// The word that starts a check's line.
    pub(crate) fn cli_doctor_status(self, status: &str) -> &'static str {
        match status {
            "pass" => self.pick("ok", "通过"),
            "warning" => self.pick("warn", "警告"),
            "fail" => self.pick("fail", "失败"),
            _ => "-",
        }
    }

    pub(crate) fn cli_doctor_verdict(self, total: usize, warnings: usize, failed: usize) -> String {
        match (self.locale, failed, warnings) {
            (Locale::EnUs, 0, 0) => format!("Orchbee OK: all {total} checks passed.\n"),
            (Locale::ZhCn, 0, 0) => format!("Orchbee 正常：{total} 项检查全部通过。\n"),
            (Locale::EnUs, 0, _) => format!("Orchbee usable with {warnings} warning(s).\n"),
            (Locale::ZhCn, 0, _) => format!("Orchbee 可用，有 {warnings} 项警告。\n"),
            (Locale::EnUs, _, _) => format!("Orchbee has problems: {failed} check(s) failed.\n"),
            (Locale::ZhCn, _, _) => format!("Orchbee 有问题：{failed} 项检查失败。\n"),
        }
    }

    pub(crate) const fn cli_doctor_details_hint(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Details: orchbee doctor --format json\n",
            Locale::ZhCn => "详情：orchbee doctor --format json\n",
        }
    }

    pub(crate) const fn cli_doctor_failed(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "One or more checks failed.\n",
            Locale::ZhCn => "一项或多项检查失败。\n",
        }
    }

    pub(crate) fn cli_doctor_delivery_unknown(self) -> &'static str {
        self.pick(
            "the installed release could not be verified",
            "无法验证已安装版本",
        )
    }

    pub(crate) fn cli_doctor_running(self, version: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("running · {version}"),
            Locale::ZhCn => format!("正在运行 · {version}"),
        }
    }

    pub(crate) fn cli_doctor_not_running(self) -> &'static str {
        self.pick("not running", "未运行")
    }

    /// Why a Session check failed: a phrase where one exists, then the reason token.
    pub(crate) fn cli_doctor_session_failure(self, reason: &str) -> String {
        let phrase = match reason {
            "session-too-old" => {
                Some(self.pick("too old for this version", "版本过旧，无法与本版本配合"))
            }
            "client-too-old" => Some(self.pick("newer than this version", "版本比本程序新")),
            "environment-mismatch" => {
                Some(self.pick("belongs to another environment", "属于另一环境"))
            }
            "fingerprint-mismatch" => Some(self.pick("is a different build", "是不同的构建")),
            "denied" => Some(self.pick("access denied", "无权访问")),
            "timed-out" => Some(self.pick("did not answer in time", "未及时应答")),
            "protocol-invalid" => Some(self.pick("answered with an invalid reply", "应答无效")),
            _ => None,
        };
        phrase.map_or_else(
            || reason.to_owned(),
            |phrase| format!("{phrase} ({reason})"),
        )
    }

    /// Why the runtime folder warns while no Session answers, by advisory token.
    pub(crate) fn cli_doctor_runtime_advisory(self, advisory: &str) -> &'static str {
        match advisory {
            "activation-recorded" => self.pick(
                "an earlier start left its activation record",
                "上次启动留下了激活记录",
            ),
            "socket-residue" => self.pick(
                "an earlier Session left its sockets",
                "上次会话留下了套接字",
            ),
            "pipe-name-present" => self.pick("a Session pipe name is present", "存在会话管道名称"),
            "recovery-blocked" => self.pick(
                "a process from an earlier start is still running",
                "之前启动的进程仍在运行",
            ),
            _ => self.pick(
                "saved Session records this version cannot use",
                "存在本版本无法使用的会话记录",
            ),
        }
    }

    const fn pick(self, en: &'static str, zh: &'static str) -> &'static str {
        match self.locale {
            Locale::EnUs => en,
            Locale::ZhCn => zh,
        }
    }
}
