use super::Locale;
#[cfg(windows)]
use crate::cli::escape::escape;
use crate::invocation::CommandPath;
#[cfg(windows)]
use orchbee_workbench_protocol::{AgentManifestsOperation, AgentManifestsOutcome};
impl Locale {
    pub(crate) fn cli_agent_manifests_help(self, path: CommandPath) -> &'static str {
        match (self, path) {
            (Self::EnUs, CommandPath::SiteAgentManifests) => {
                "orchbee site agent-manifests [--format text|json]\n\nRead the running Windows Site's installed detection rules, source, version, path and warnings. This does not reread files or fetch updates.\n"
            }
            (Self::ZhCn, CommandPath::SiteAgentManifests) => {
                "orchbee site agent-manifests [--format text|json]\n\n读取运行中 Windows Site 已安装的识别规则、来源、版本、路径及警告。不会重新读取文件或下载更新。\n"
            }
            (Self::EnUs, CommandPath::SiteReloadAgentManifests) => {
                "orchbee site reload-agent-manifests [--request-id <operation-id>] [--format text|json]\n\nReload local detection rules in the running Windows Site and reset screen stabilization. Wait up to five seconds; pending/unknown returns exit 8 and retains the operation ID. Query site agent-manifests-status <id> after interruption. Reusing an accepted ID never reloads again. Receipts last for this Site generation (maximum 1,024); no remote fetch.\n"
            }
            (Self::ZhCn, CommandPath::SiteReloadAgentManifests) => {
                "orchbee site reload-agent-manifests [--request-id <operation-id>] [--format text|json]\n\n在运行中 Windows Site 重载本地识别规则并重置屏幕稳定状态。最多等待五秒；待处理或结果未知时退出码为 8，并保留操作 ID。中断后使用 site agent-manifests-status <id> 查询。复用已接纳的 ID 不会再次重载。回执保留至此 Site 代次结束（最多 1,024 条）；不会远程下载。\n"
            }
            (Self::EnUs, CommandPath::SiteUpdateAgentManifests) => {
                "orchbee site update-agent-manifests [--request-id <operation-id>] [--format text|json]\n\nDownload detection rules from the configured catalog and activate eligible cached rules. Wait up to five seconds; exit 8 retains the operation ID for site agent-manifests-status. Reusing an ID does not repeat the download. Partial failure returns a nonzero exit; cache results and activation are separate.\n"
            }
            (Self::ZhCn, CommandPath::SiteUpdateAgentManifests) => {
                "orchbee site update-agent-manifests [--request-id <operation-id>] [--format text|json]\n\n从配置的目录源下载识别规则并启用符合条件的缓存规则。最多等待五秒；退出码 8 会保留操作 ID，可用 site agent-manifests-status 查询。复用 ID 不会重复下载。部分失败返回非零退出码；缓存结果和启用结果分别显示。\n"
            }
            (Self::EnUs, _) => {
                "orchbee site agent-manifests-status <operation-id> [--format text|json]\n\nRead one retained reload or update receipt from the running Windows Site without resubmitting it. Unknown IDs are not recorded; receipts do not survive Site restart.\n"
            }
            (Self::ZhCn, _) => {
                "orchbee site agent-manifests-status <operation-id> [--format text|json]\n\n读取运行中 Windows Site 保留的重载或更新回执，不重新提交。未知 ID 表示无记录；回执不跨 Site 重启保留。\n"
            }
        }
    }
    #[cfg(windows)]
    pub(crate) fn cli_agent_manifests(
        self,
        operation_id: Option<&str>,
        outcome: Option<&AgentManifestsOutcome>,
        unknown: Option<&str>,
    ) -> String {
        let zh = matches!(self, Self::ZhCn);
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
            Some(AgentManifestsOutcome::Operation {status:AgentManifestsOperation::Unknown {reason},..})=>text.push_str(&format!("{}: {:?}\n",if zh {"更新结果未知，请检查缓存；本次 Site 已停用后续更新"} else {"Update outcome unknown; inspect the cache. Further updates are disabled for this Site"},reason)),
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

#[cfg(windows)]
impl Locale {
    fn cli_agent_manifest_update(
        self,
        report: &orchbee_workbench_protocol::AgentManifestUpdateReport,
    ) -> String {
        let zh = matches!(self, Self::ZhCn);
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
