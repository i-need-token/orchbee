use crate::{cli::escape::escape, invocation::CommandPath, localization::Locale};
use serde_json::Value;

impl Locale {
    pub(crate) fn cli_worktree_result(self, result: &Value) -> String {
        let id = result["operationId"].as_str().expect("operation identity");
        let observation = &result["observation"];
        let reply = &observation["reply"];
        let outcome = &reply["outcome"];
        let status = &outcome["status"];
        let state = status["kind"]
            .as_str()
            .or_else(|| outcome["kind"].as_str())
            .unwrap_or_else(|| observation["kind"].as_str().expect("observation kind"));
        let label = match (self, state) {
            (Self::EnUs, "pending") => "In progress",
            (Self::ZhCn, "pending") => "进行中",
            (Self::EnUs, "opened") => "Workspace associated",
            (Self::ZhCn, "opened") => "已关联 Workspace",
            (Self::EnUs, "removed") => "Worktree removed",
            (Self::ZhCn, "removed") => "已移除 Worktree",
            (Self::EnUs, "failed") => "Operation failed",
            (Self::ZhCn, "failed") => "操作失败",
            (Self::EnUs, "not-found") => "Operation not recorded",
            (Self::ZhCn, "not-found") => "未找到操作记录",
            (Self::EnUs, "conflict") => "Operation identity or attempt conflict",
            (Self::ZhCn, "conflict") => "操作标识或尝试序号冲突",
            (Self::EnUs, "denied") => "Permission denied",
            (Self::ZhCn, "denied") => "权限不足",
            (Self::EnUs, "unknown") => "Submission outcome unknown",
            (Self::ZhCn, "unknown") => "提交结果不确定",
            (Self::EnUs, _) => "Unavailable",
            (Self::ZhCn, _) => "不可用",
        };
        let mut text = format!("{label}\n");
        let setup = &result["setup"];
        let fields = [
            (
                "Created Project",
                "已创建项目",
                setup["project"]["result"]["projectId"].as_str(),
            ),
            ("Setup stage", "准备阶段", setup["stage"].as_str()),
            (
                "Setup failure",
                "准备失败",
                setup["failure"]["kind"].as_str(),
            ),
            (
                "Setup request",
                "准备请求",
                setup["failure"]["details"]["requestId"].as_str(),
            ),
            ("Operation", "操作", Some(id)),
            ("Phase", "阶段", status["phase"].as_str()),
            (
                "Workspace",
                "Workspace",
                status["workspaceId"]
                    .as_str()
                    .or_else(|| status["progress"]["workspace"]["workspaceId"].as_str()),
            ),
            (
                "Workspace state",
                "Workspace 状态",
                status["progress"]["workspace"]["kind"].as_str(),
            ),
            (
                "Path",
                "路径",
                status["path"]
                    .as_str()
                    .or_else(|| status["checkout"]["path"].as_str())
                    .or_else(|| status["progress"]["path"].as_str()),
            ),
            (
                "Branch",
                "分支",
                status["checkout"]["branch"]
                    .as_str()
                    .or_else(|| status["progress"]["checkout"]["branch"].as_str()),
            ),
            (
                "Git effect",
                "Git 影响",
                status["progress"]["gitEffect"].as_str(),
            ),
            (
                "Terminal",
                "终端",
                status["terminalOutcome"]["kind"].as_str(),
            ),
            (
                "Reason",
                "原因",
                status["reason"]["error"]
                    .as_str()
                    .or_else(|| status["reason"]["kind"].as_str())
                    .or_else(|| outcome["reason"].as_str())
                    .or_else(|| observation["reason"].as_str()),
            ),
        ];
        for (en, zh, value) in fields {
            if let Some(value) = value {
                text.push_str(&format!(
                    "{}: {}\n",
                    if self == Self::EnUs { en } else { zh },
                    escape(value)
                ));
            }
        }
        if let Some(bytes) = status["reason"]["diagnostics"].as_array() {
            let bytes: Vec<u8> = bytes
                .iter()
                .filter_map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect();
            if !bytes.is_empty() {
                text.push_str(&format!(
                    "Git: {}\n",
                    escape(&String::from_utf8_lossy(&bytes))
                ));
            }
        }
        if let Some(wait) = result["wait"].as_str() {
            text.push_str(match (self, wait) {
                (Self::EnUs, "detached") => "The Server continues in the background.\n",
                (Self::ZhCn, "detached") => "Server 将继续在后台执行。\n",
                (Self::EnUs, "cancelled") => {
                    "Waiting cancelled. An accepted operation continues on the Server.\n"
                }
                (Self::ZhCn, "cancelled") => "已取消等待；Server 继续执行已接受的操作。\n",
                (Self::EnUs, "timed-out") => {
                    "Waiting timed out. An accepted operation continues on the Server.\n"
                }
                (Self::ZhCn, "timed-out") => "等待超时；Server 继续执行已接受的操作。\n",
                (Self::EnUs, "observation-failed") => {
                    "Observation failed. The last known operation state is retained.\n"
                }
                (Self::ZhCn, "observation-failed") => "观测失败；保留最近已知的操作状态。\n",
                (Self::EnUs, _) => "Connection lost. The last observed state is retained below.\n",
                (Self::ZhCn, _) => "连接中断；保留最近观察到的状态。\n",
            });
        }
        if state == "pending" || state == "unknown" || result["wait"].is_string() {
            text.push_str(&format!("orchbee worktree status {id}\n"));
        }
        if status["resumable"] == true
            && let Some(attempt) = outcome["attempt"].as_u64()
        {
            text.push_str(&format!(
                "orchbee worktree resume {id} --generation {attempt} --confirm\n"
            ));
        }
        if let Some(post) = result.get("postOpen") {
            text.push_str(match (self, post["outcome"].as_str()) {
                (Self::EnUs, Some("completed")) => "TUI focus completed.\n",
                (Self::ZhCn, Some("completed")) => "TUI 聚焦已完成。\n",
                (Self::EnUs, _) => "TUI focus incomplete; the Worktree result above is retained.\n",
                (Self::ZhCn, _) => "TUI 聚焦未完成；保留上方 Worktree 结果。\n",
            });
            if let Some(failure) = post["failure"].as_str() {
                text.push_str(&format!(
                    "{}: {}\n",
                    if self == Self::EnUs {
                        "Focus failure"
                    } else {
                        "聚焦失败"
                    },
                    escape(failure)
                ));
            }
            if post["focus"].is_object() {
                text.push_str(&self.cli_client_control(&post["focus"]));
            }
        }
        text
    }

    pub(super) fn cli_worktree_help(self, path: CommandPath) -> &'static str {
        match (self, path) {
            (Self::EnUs, CommandPath::WorktreeCreate) => {
                "orchbee worktree create [--project <target>] [--workspace <target>|--cwd <path>] [--branch <name>] [--base <ref>] [--path <path>] [--label <label>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json] [--client <id>] [--focus|--no-focus]\n\nWindows: create a Git worktree, associate a Workspace, and launch its terminal.\nSource: explicit --client overrides caller; otherwise caller, unique current ready TUI, then CLI cwd only with no caller/Clients. Missing Workspace directory uses only the proven terminal root cwd.\nProject may be omitted when inferred from the Workspace or when exactly one exists. With zero Projects, create one Default first; multiple Projects without a source are ambiguous. A first-Project attempt uses JSON v3 with a separate setup receipt.\nWindows paths: absolute, caller-relative or ~ (USERPROFILE, HOMEDRIVE+HOMEPATH, HOME). No shell expansion.\nDefault branch: worktree/<operation-id>; default base: HEAD.\nDefault path: sibling <repository>-worktrees/<branch-slug>. Existing branches ignore --base.\n--trust-repository grants Git trust only to this operation. It does not change Git configuration.\n--focus switches an existing TUI after opening; default --no-focus. Focus rejects --detach; a focus failure retains the Worktree result (JSON v2).\nTimeout: 1–3600000 ms, default 120000. --detach returns the retained operation immediately.\nCtrl+C stops waiting; status reads the continuing operation. Do not replay an uncertain mutation.\nOptional terminal geometry: --cols <n> --rows <n> --cell-width <n> --cell-height <n> (all four).\n"
            }
            (Self::ZhCn, CommandPath::WorktreeCreate) => {
                "orchbee worktree create [--project <target>] [--workspace <target>|--cwd <路径>] [--branch <名称>] [--base <引用>] [--path <路径>] [--label <标签>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--format text|json] [--client <id>] [--focus|--no-focus]\n\nWindows：创建 Git worktree，关联 Workspace 并启动其终端。\n来源：显式 --client 优先于调用者；否则依次为调用者、唯一当前就绪 TUI、无调用者和 Client 时的 CLI cwd。Workspace 无主目录时只用已证明终端的根进程 cwd。\n可从 Workspace 推导项目，或只有一个项目时，可省略 --project。零项目时先创建默认项目；无来源且多项目时拒绝歧义。首次项目创建使用 JSON v3，独立保留 setup 回执。\nWindows 路径支持绝对路径、相对调用者目录或 ~（USERPROFILE、HOMEDRIVE+HOMEPATH、HOME）。不做 shell 展开。\n默认分支：worktree/<操作标识>；默认基准：HEAD。\n默认路径：同级 <仓库名>-worktrees/<分支路径名>。已有分支忽略 --base。\n--trust-repository 仅对此操作授予 Git 信任，不修改 Git 配置。\n--focus 在打开后切换已有 TUI，默认 --no-focus。聚焦不可与 --detach 混用；聚焦失败保留 Worktree 结果（JSON v2）。\n超时：1–3600000 毫秒，默认 120000。--detach 立即返回保留的操作。\nCtrl+C 停止等待；status 可查询继续执行的操作。不要重放结果不确定的变更。\n可选终端尺寸：--cols <n> --rows <n> --cell-width <n> --cell-height <n>（四项全填）。\n"
            }
            (Self::EnUs, CommandPath::WorktreeOpen) => {
                "orchbee worktree open (--path <path>|--branch <name>) [--project <target>] [--workspace <target>|--cwd <path>] [--label <label>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json] [--client <id>] [--focus|--no-focus]\n\nWindows: inspect an existing checkout and reuse or create its Workspace. No Git checkout is created.\nMain and linked checkouts are supported; ambiguous branch selection requires --path.\nSource/project defaults, absolute/relative/~ paths, terminal geometry and waiting follow worktree create.\n--focus switches an existing TUI after opening; default --no-focus. Focus cannot be detached.\n--label also renames a reused Workspace. Inspect partial launch outcomes in the result.\n"
            }
            (Self::ZhCn, CommandPath::WorktreeOpen) => {
                "orchbee worktree open (--path <路径>|--branch <名称>) [--project <target>] [--workspace <target>|--cwd <路径>] [--label <标签>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--format text|json] [--client <id>] [--focus|--no-focus]\n\nWindows：检查已有检出目录，复用或创建其 Workspace；不会新建 Git 检出目录。\n支持主检出及关联检出；分支选择有歧义时须使用 --path。\n来源、项目默认值、绝对／相对／~ 路径、终端尺寸和等待方式与 worktree create 一致。\n--focus 在打开后切换已有 TUI；默认 --no-focus，聚焦不可与 --detach 混用。\n--label 同样会重命名复用的 Workspace；请检查结果中的终端启动状态。\n"
            }
            (Self::EnUs, CommandPath::WorktreeRemove) => {
                "orchbee worktree remove --workspace <target> --confirm [--project <target>] [--force] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]\n\nWindows: inspect the linked checkout, close its Workspace, then remove it through Git.\nDirty worktrees are refused before closing unless --force explicitly permits discarding changes.\nMain/bare checkouts and checkouts used by another Workspace are refused. Branches are preserved.\nA later Git failure retains the closed Workspace fact; status shows whether an explicit resume is possible.\nTimeout defaults to 120000 ms. Ctrl+C stops waiting; it does not undo accepted work.\n"
            }
            (Self::ZhCn, CommandPath::WorktreeRemove) => {
                "orchbee worktree remove --workspace <target> --confirm [--project <target>] [--force] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--format text|json]\n\nWindows：检查关联检出目录，关闭其 Workspace，再通过 Git 移除目录。\n脏目录会在关闭前拒绝；--force 明确允许丢弃改动。\n拒绝主检出、裸仓库及其他 Workspace 占用的目录；保留分支。\n后续 Git 失败会保留 Workspace 已关闭的事实；status 显示能否显式恢复。\n默认超时 120000 毫秒。Ctrl+C 停止等待，不撤销已接受的操作。\n"
            }
            (Self::EnUs, CommandPath::WorktreeStatus) => {
                "orchbee worktree status <operation-id> [--format text|json]\n\nRead a retained operation without mutation or Site activation. IDs survive CLI disconnection, not Server restart.\nThe result includes attempt, Git effects, Workspace state and recovery eligibility.\n"
            }
            (Self::ZhCn, CommandPath::WorktreeStatus) => {
                "orchbee worktree status <操作标识> [--format text|json]\n\n只读查询保留的操作，不执行变更或激活 Site。记录可跨 CLI 断线保留，但不跨 Server 重启。\n结果包含尝试序号、Git 影响、Workspace 状态及能否恢复。\n"
            }
            (Self::EnUs, _) => {
                "orchbee worktree resume <operation-id> --generation <attempt> --confirm [--detach] [--timeout <ms>] [--format text|json]\n\nExplicitly resume a known, resumable failed operation using the attempt from status.\nA changed attempt, Worker incarnation or unproven process settlement is refused.\nGit creation is observed before any recovery; an already closed Workspace stays closed.\nConfirmation is required because the retained operation can remove user work.\n"
            }
            (Self::ZhCn, _) => {
                "orchbee worktree resume <操作标识> --generation <尝试序号> --confirm [--detach] [--timeout <毫秒>] [--format text|json]\n\n使用 status 返回的尝试序号，显式恢复已知且可恢复的失败操作。\n尝试序号、Worker 实例已变化或进程收尾无法证明时拒绝恢复。\n恢复前先观察 Git 创建结果；已关闭的 Workspace 保持关闭。\n保留的操作可能删除用户工作，因此必须确认。\n"
            }
        }
    }
}

#[cfg(any(windows, test))]
impl Locale {
    pub(crate) fn cli_worktree_query(
        self,
        outcome: &orchbee_workbench_protocol::WorkbenchWorktreeQueryOutcome,
    ) -> String {
        use orchbee_workbench_protocol::{
            WorkbenchWorktreeQueryOutcome as O, WorktreeEntryText, WorktreeQueryFailure as F,
        };
        let word = |en, zh| match self {
            Self::EnUs => en,
            Self::ZhCn => zh,
        };
        match outcome {
            O::Found {
                source, entries, ..
            } => {
                let mut text = format!(
                    "{}: {}\n{}: {}\n",
                    word("Repository", "仓库"),
                    escape(source.repository_root.as_str()),
                    word("Source checkout", "来源检出"),
                    escape(source.checkout_path.as_str())
                );
                for entry in entries {
                    let checkout = &entry.checkout;
                    let WorktreeEntryText::Utf8(path) = &checkout.path else {
                        unreachable!("validated UTF-8 query")
                    };
                    let branch = match &checkout.branch {
                        Some(WorktreeEntryText::Utf8(branch)) => branch.as_str(),
                        _ if checkout.bare => word("bare", "裸仓库"),
                        _ => word("detached HEAD", "分离 HEAD"),
                    };
                    let kind = if entry.linked {
                        word("linked", "关联检出")
                    } else {
                        word("main", "主检出")
                    };
                    text.push_str(&format!(
                        "\n{}  [{}]  {}\n",
                        escape(path),
                        kind,
                        escape(branch)
                    ));
                    if checkout.locked {
                        text.push_str(word("  Locked\n", "  已锁定\n"));
                    }
                    if checkout.prunable {
                        text.push_str(word("  Prunable\n", "  可清理\n"));
                    }
                    if entry.workspaces.is_empty() {
                        text.push_str(word("  No associated Workspace\n", "  未关联 Workspace\n"));
                    }
                    for workspace in &entry.workspaces {
                        text.push_str(&format!(
                            "  {}: {} ({})  {}: {}\n",
                            word("Workspace", "Workspace"),
                            escape(workspace.label.as_str()),
                            workspace.workspace_id.as_str(),
                            word("Project", "项目"),
                            workspace.project_id.as_str()
                        ));
                    }
                }
                text
            }
            O::Failed { reason } => {
                let message = match reason {
                    F::NotFound {} => word("Source not found", "未找到来源"),
                    F::Denied {} => word("Permission denied", "权限不足"),
                    F::Unsupported {} => word("Unsupported platform", "当前平台不支持"),
                    F::SourceChanged {} => word(
                        "Source changed during the query; retry",
                        "查询期间来源发生变化，请重试",
                    ),
                    F::Unavailable { .. } => word(
                        "Worker or query slot unavailable",
                        "Worker 或查询槽位不可用",
                    ),
                    F::GitFailed { .. } => word(
                        "Git query failed; check the repository and its ownership",
                        "Git 查询失败，请检查仓库及其所有权",
                    ),
                    F::SpawnUnavailable { .. } => {
                        word("Git process owner unavailable", "Git 进程所有者不可用")
                    }
                    F::SpawnFailed { .. } => word("Could not start Git", "无法启动 Git"),
                    F::TooLarge { .. } => word(
                        "Worktree listing exceeds its output limit",
                        "Worktree 列表超出输出上限",
                    ),
                    F::Timeout {} => word("Worktree query timed out", "Worktree 查询超时"),
                    F::OwnerIo { .. } => word("Git query I/O failed", "Git 查询 I/O 失败"),
                    F::TransportLoss {} => {
                        word("Connection lost during the query", "查询期间连接中断")
                    }
                    F::MalformedListing {} => word(
                        "Git returned an invalid worktree listing",
                        "Git 返回了无效的 Worktree 列表",
                    ),
                };
                format!("{message}\n")
            }
        }
    }
}
