//! Formats manifest inspection and update outcomes with CLI escaping; collection and mutation remain with the feature runner.
use super::Locale;
use crate::cli::escape::escape;
use crate::invocation::CommandPath;
use orchbee_workbench_protocol::{AgentManifestsOperation, AgentManifestsOutcome};
impl super::CliText {
    pub(crate) fn cli_agent_manifests_help(
        self,
        path: CommandPath,
    ) -> crate::cli_text::HelpDetails {
        match (self.locale, path) {
            (Locale::EnUs, CommandPath::ServerAgentManifests) => crate::cli_text::HelpDetails::new(
                "orchbee server agent-manifests [--format text|json]",
                "Read the running Windows session's installed detection rules, source, version, path and warnings. Includes automatic-check policy and its latest result. This does not reread files or fetch updates.",
            ),
            (Locale::ZhCn, CommandPath::ServerAgentManifests) => crate::cli_text::HelpDetails::new(
                "orchbee server agent-manifests [--format text|json]",
                "读取运行中 Windows 会话已安装的识别规则、来源、版本、路径及警告。包含自动检查策略及最近结果。不会重新读取文件或下载更新。",
            ),
            (Locale::EnUs, CommandPath::ServerReloadAgentManifests) => {
                crate::cli_text::HelpDetails::new(
                    "orchbee server reload-agent-manifests [--request-id <operation-id>] [--format text|json]",
                    "Reload local detection rules and update.manifest_check from the frozen configuration path in the running Windows session; reset screen stabilization. Invalid configuration retains the previous policy and rules. Wait up to five seconds; pending/unknown returns exit 8 and retains the operation ID. Query server agent-manifests-status <id> after interruption. Reusing an accepted ID never reloads again. Receipts last for this session generation (maximum 1,024); no remote fetch.",
                )
            }
            (Locale::ZhCn, CommandPath::ServerReloadAgentManifests) => {
                crate::cli_text::HelpDetails::new(
                    "orchbee server reload-agent-manifests [--request-id <operation-id>] [--format text|json]",
                    "在运行中 Windows 会话从冻结配置路径重载本地识别规则和 update.manifest_check，并重置屏幕稳定状态。配置无效时保留旧策略和规则。最多等待五秒；待处理或结果未知时退出码为 8，并保留操作 ID。中断后使用 server agent-manifests-status <id> 查询。复用已接纳的 ID 不会再次重载。回执保留至此会话代次结束（最多 1,024 条）；不会远程下载。",
                )
            }
            (Locale::EnUs, CommandPath::ServerUpdateAgentManifests) => {
                crate::cli_text::HelpDetails::new(
                    "orchbee server update-agent-manifests [--request-id <operation-id>] [--format text|json]",
                    "Download detection rules from the configured catalog and activate eligible cached rules. Wait up to five seconds; exit 8 retains the operation ID for server agent-manifests-status. Reusing an ID does not repeat the download. Partial failure returns a nonzero exit; cache results and activation are separate.",
                )
            }
            (Locale::ZhCn, CommandPath::ServerUpdateAgentManifests) => {
                crate::cli_text::HelpDetails::new(
                    "orchbee server update-agent-manifests [--request-id <operation-id>] [--format text|json]",
                    "从配置的目录源下载识别规则并启用符合条件的缓存规则。最多等待五秒；退出码 8 会保留操作 ID，可用 server agent-manifests-status 查询。复用 ID 不会重复下载。部分失败返回非零退出码；缓存结果和启用结果分别显示。",
                )
            }
            (Locale::EnUs, _) => crate::cli_text::HelpDetails::new(
                "orchbee server agent-manifests-status <operation-id> [--format text|json]",
                "Read one retained reload or update receipt from the running Windows session without resubmitting it. Unknown IDs are not recorded; receipts do not survive session restart.",
            ),
            (Locale::ZhCn, _) => crate::cli_text::HelpDetails::new(
                "orchbee server agent-manifests-status <operation-id> [--format text|json]",
                "读取运行中 Windows 会话保留的重载或更新回执，不重新提交。未知 ID 表示无记录；回执不跨会话重启保留。",
            ),
        }
    }
    pub(crate) fn cli_agent_manifests(
        self,
        operation_id: Option<&str>,
        outcome: Option<&AgentManifestsOutcome>,
        unknown: Option<&str>,
    ) -> String {
        let zh = matches!(self.locale, Locale::ZhCn);
        let mut text = String::new();
        if let Some(id) = operation_id {
            text.push_str(&format!(
                "{}: {}\n",
                if zh { "操作 ID" } else { "Operation ID" },
                escape(id)
            ));
        }
        match outcome {
            Some(AgentManifestsOutcome::Current { snapshot }) => {
                text.push_str(&format!(
                    "{}: {}  {}: {}\n{}: {}\n",
                    if zh {
                        "规则修订"
                    } else {
                        "Manifest revision"
                    },
                    snapshot.revision,
                    if zh { "引擎" } else { "Engine" },
                    snapshot.engine_version,
                    if zh {
                        "配置目录"
                    } else {
                        "Configuration root"
                    },
                    escape(snapshot.configuration_root.as_deref().unwrap_or("-"))
                ));
                if let Some(maintenance)=&snapshot.maintenance {
                    text.push_str(&format!("{}: {}\n",if zh {"更新目录源"} else {"Update catalog"},escape(maintenance.source_url.as_deref().unwrap_or("-"))));
                    if let Some(error)=maintenance.source_error { text.push_str(&format!("{}: {:?}\n",if zh {"目录源错误"} else {"Catalog error"},error)); }
                    if let Some(error)=maintenance.status_error { text.push_str(&format!("{}: {:?}\n",if zh {"缓存状态错误"} else {"Cache status error"},error)); }
                    if let Some(automatic)=&maintenance.automatic {
                        text.push_str(&self.cli_agent_manifest_automatic(automatic));
                    }
                    if let Some(report)=&maintenance.last_update {text.push_str(&self.cli_agent_manifest_update(report));}
                }
                for row in &snapshot.manifests {
                    text.push_str(&format!(
                        "{}  {}={}  {}={}  {}={}\n",
                        escape(&row.agent),
                        if zh { "版本" } else { "version" },
                        escape(row.version.as_deref().unwrap_or("-")),
                        if zh { "来源" } else { "source" },
                        row.manifest.source.as_str(),
                        if zh { "路径" } else { "path" },
                        escape(row.manifest.path.as_deref().unwrap_or("-"))
                    ));
                    if let Some(version) = &row.manifest.cached_remote_version {
                        text.push_str(&format!(
                            "  {}: {}  {}: {}\n",
                            if zh { "缓存版本" } else { "Cached version" },
                            escape(version),
                            if zh {
                                "被本地覆盖"
                            } else {
                                "Shadowed locally"
                            },
                            row.manifest.local_override_shadowing_remote
                        ));
                    }
                    for warning in &row.manifest.warnings {
                        text.push_str(&format!(
                            "  {}: {} / {:?}\n",
                            if zh { "警告" } else { "Warning" },
                            warning.source.as_str(),
                            warning.kind
                        ));
                    }
                }
            }
            Some(AgentManifestsOutcome::Operation {status:AgentManifestsOperation::Updated {report,activation},..})=> {
                text.push_str(&self.cli_agent_manifest_update(report));
                use orchbee_workbench_protocol::AgentManifestActivation;
                match activation {
                    AgentManifestActivation::Applied {revision}=>text.push_str(&format!("{}: {}\n",if zh {"已启用规则修订"} else {"Activated manifest revision"},revision)),
                    AgentManifestActivation::NotAttempted {}=>text.push_str(if zh {"未启用缓存规则。\n"} else {"Cached rules were not activated.\n"}),
                    AgentManifestActivation::Failed {reason}=>text.push_str(&format!("{}: {:?}\n",if zh {"规则启用失败"} else {"Activation failed"},reason)),
                }
            },
            Some(AgentManifestsOutcome::Operation {status:AgentManifestsOperation::Unknown {reason},..})=>text.push_str(&format!("{}: {:?}\n",if zh {"更新结果未知，请检查缓存；本次会话已停用后续更新"} else {"Update outcome unknown; inspect the cache. Further updates are disabled for this session"},reason)),
            Some(AgentManifestsOutcome::Operation {
                status: AgentManifestsOperation::Applied { revision },
                ..
            }) => text.push_str(&format!(
                "{}: {}\n",
                if zh {
                    "已安装规则修订"
                } else {
                    "Installed manifest revision"
                },
                revision
            )),
            Some(AgentManifestsOutcome::Operation {
                status: AgentManifestsOperation::Pending {},
                ..
            }) => text.push_str(if zh {
                "操作待完成；可使用操作 ID 查询状态。\n"
            } else {
                "Operation pending; query status with the operation ID.\n"
            }),
            Some(
                AgentManifestsOutcome::Failed { reason }
                | AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Failed { reason },
                    ..
                },
            ) => text.push_str(&format!(
                "{}: {:?}\n",
                if zh {
                    "规则操作失败"
                } else {
                    "Manifest operation failed"
                },
                reason
            )),
            None => text.push_str(&format!(
                "{}: {}\n",
                if zh {
                    "结果未知"
                } else {
                    "Outcome unknown"
                },
                escape(unknown.unwrap_or("unknown"))
            )),
        }
        text
    }
}

impl super::CliText {
    fn cli_agent_manifest_update(
        self,
        report: &orchbee_workbench_protocol::AgentManifestUpdateReport,
    ) -> String {
        let zh = matches!(self.locale, Locale::ZhCn);
        let mut text = format!(
            "{}: {}  {}: {}\n{}: {}\n",
            if zh {
                "检查时间（Unix 秒）"
            } else {
                "Checked at (Unix seconds)"
            },
            report.checked_unix_seconds,
            if zh {
                "状态已保存"
            } else {
                "Status saved"
            },
            report.status_saved,
            if zh {
                "本次目录源"
            } else {
                "Checked catalog"
            },
            escape(&report.catalog_url)
        );
        if let Some(error) = report.failure {
            text.push_str(&format!(
                "{}: {:?}\n",
                if zh { "更新失败" } else { "Update failed" },
                error
            ));
        }
        for row in &report.agents {
            let disposition = match (zh, row.result) {
                (true, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Updated) => {
                    "已写入缓存"
                }
                (false, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Updated) => {
                    "cached"
                }
                (true, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Current) => {
                    "缓存已是最新"
                }
                (false, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Current) => {
                    "cache current"
                }
                (true, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Failed) => {
                    "失败"
                }
                (false, orchbee_workbench_protocol::AgentManifestUpdateDisposition::Failed) => {
                    "failed"
                }
            };
            text.push_str(&format!(
                "{}: {}  {}={}  {}={}\n",
                escape(&row.agent),
                disposition,
                if zh { "尝试版本" } else { "attempted" },
                escape(row.attempted_version.as_deref().unwrap_or("-")),
                if zh { "缓存版本" } else { "cached" },
                escape(row.cached_version.as_deref().unwrap_or("-"))
            ));
            if let Some(error) = row.error {
                text.push_str(&format!(
                    "  {}: {:?}\n",
                    if zh { "原因" } else { "Reason" },
                    error
                ));
            }
        }
        text
    }
}

impl super::CliText {
    fn cli_agent_manifest_automatic(
        self,
        automatic: &orchbee_workbench_protocol::AgentManifestAutomatic,
    ) -> String {
        let zh = matches!(self.locale, Locale::ZhCn);
        let mut text = format!(
            "{}: {}  {}={}  {}={:?}  {}={}  {}={}\n",
            if zh {
                "自动检查（每 30 分钟）"
            } else {
                "Automatic checks (every 30 minutes)"
            },
            automatic.enabled,
            if zh { "配置开关" } else { "configured" },
            automatic.manifest_check,
            if zh { "设置来源" } else { "setting source" },
            automatic.manifest_check_source,
            if zh { "后台策略" } else { "background" },
            automatic.background_enabled,
            if zh { "发布构建" } else { "release build" },
            automatic.release_build
        );
        if let Some(operation) = automatic.last_operation.as_deref() {
            text.push_str(if zh {
                "最近自动检查：\n"
            } else {
                "Latest automatic check:\n"
            });
            match operation {
                AgentManifestsOperation::Pending {} => text.push_str(if zh {
                    "检查进行中。\n"
                } else {
                    "Check pending.\n"
                }),
                AgentManifestsOperation::Updated { report, activation } => {
                    text.push_str(&self.cli_agent_manifest_update(report));
                    use orchbee_workbench_protocol::AgentManifestActivation;
                    match activation {
                        AgentManifestActivation::Applied { revision } => text.push_str(&format!(
                            "{}: {}\n",
                            if zh {
                                "已启用规则修订"
                            } else {
                                "Activated manifest revision"
                            },
                            revision
                        )),
                        AgentManifestActivation::NotAttempted {} => text.push_str(if zh {
                            "未启用缓存规则。\n"
                        } else {
                            "Cached rules were not activated.\n"
                        }),
                        AgentManifestActivation::Failed { reason } => text.push_str(&format!(
                            "{}: {:?}\n",
                            if zh {
                                "规则启用失败"
                            } else {
                                "Activation failed"
                            },
                            reason
                        )),
                    }
                }
                AgentManifestsOperation::Unknown { reason } => text.push_str(&format!(
                    "{}: {:?}\n",
                    if zh {
                        "结果未知"
                    } else {
                        "Outcome unknown"
                    },
                    reason
                )),
                AgentManifestsOperation::Failed { reason } => text.push_str(&format!(
                    "{}: {:?}\n",
                    if zh { "失败" } else { "Failed" },
                    reason
                )),
                AgentManifestsOperation::Applied { .. } => {}
            }
        }
        text
    }
}
