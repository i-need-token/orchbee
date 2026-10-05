use crate::{
    cli::{config_import::Receipt, escape::quoted},
    localization::Locale,
};

impl Locale {
    pub(crate) fn cli_config_import(self, receipt: &Receipt<'_>) -> String {
        let mut text = match (receipt.error, receipt.preview, self) {
            (Some((stage, reason)), _, Self::EnUs) => {
                format!("Configuration import failed ({stage}: {reason}).\n")
            }
            (Some((stage, reason)), _, Self::ZhCn) => {
                format!("配置导入失败（{stage}: {reason}）。\n")
            }
            (None, true, Self::EnUs) => {
                "Configuration preview prepared; no destination was opened.\n".into()
            }
            (None, true, Self::ZhCn) => "已准备配置预览，未打开目标文件。\n".into(),
            (None, false, Self::EnUs) => "New configuration created.\n".into(),
            (None, false, Self::ZhCn) => "已创建新配置。\n".into(),
        };
        for (label, path) in [
            (
                if self == Self::EnUs {
                    "Source"
                } else {
                    "来源"
                },
                receipt.source,
            ),
            (
                if self == Self::EnUs {
                    "Destination"
                } else {
                    "目标"
                },
                receipt.destination,
            ),
        ] {
            if let Some(path) = path {
                text.push_str(&format!("{label}: {}\n", quoted(path)));
            }
        }
        if let Some(plan) = receipt.plan {
            text.push_str(&match self {
                Self::EnUs => format!("Candidate: {} bytes.\n", plan.candidate_toml().len()),
                Self::ZhCn => format!("候选配置：{} 字节。\n", plan.candidate_toml().len()),
            });
            for item in plan.report().items() {
                let targets = item
                    .target_fields
                    .iter()
                    .map(|target| quoted(target))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mapped = if targets.is_empty() {
                    String::new()
                } else {
                    format!(" -> {targets}")
                };
                text.push_str(&format!(
                    "{}{mapped}: {} ({})\n",
                    quoted(&item.field),
                    item.disposition.as_str(),
                    item.reason
                ));
            }
            if plan.report().omitted_count() != 0 {
                text.push_str(&match self {
                    Self::EnUs => format!(
                        "Additional fields omitted: {}.\n",
                        plan.report().omitted_count()
                    ),
                    Self::ZhCn => format!("另有 {} 项未显示。\n", plan.report().omitted_count()),
                });
            }
        }
        if receipt.created && receipt.error.is_some() {
            text.push_str(match self {
                Self::EnUs => "Inspect the retained destination before recovery; it may be empty, partial, or complete. This invocation will not delete or retry it.\n",
                Self::ZhCn => "恢复前请检查保留的目标文件；它可能为空、不完整或完整。本次调用不会删除或重试。\n",
            });
        } else if receipt.created {
            text.push_str(match self {
                Self::EnUs => "You own the new file. Select it with ORCHBEE_CONFIG_FILE for a new Client; select your old file to roll back. Remove the candidate when no longer needed.\n",
                Self::ZhCn => "新文件已交付给你。用 ORCHBEE_CONFIG_FILE 为新 Client 选择它；选择旧文件即可回退。不再需要候选配置时可自行删除。\n",
            });
        }
        text
    }
}
