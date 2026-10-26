//! Localized worktree operations, query failures, and command help; does not inspect or mutate checkouts.

use super::failure_target;
use crate::cli::Failure;
use crate::cli::escape::native_path;
use crate::{cli::escape::escape, invocation::CommandPath, localization::Locale};
use serde_json::Value;

impl super::CliText {
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
        let label = match (self.locale, state) {
            (Locale::EnUs, "pending") => "In progress",
            (Locale::ZhCn, "pending") => "进行中",
            (Locale::EnUs, "opened") => "Workspace associated",
            (Locale::ZhCn, "opened") => "已关联工作区",
            (Locale::EnUs, "removed") => "Worktree removed",
            (Locale::ZhCn, "removed") => "已移除工作树",
            (Locale::EnUs, "failed") => "Operation failed",
            (Locale::ZhCn, "failed") => "操作失败",
            (Locale::EnUs, "not-found") => "Operation not recorded",
            (Locale::ZhCn, "not-found") => "未找到操作记录",
            (Locale::EnUs, "conflict") => "Operation identity or attempt conflict",
            (Locale::ZhCn, "conflict") => "操作标识或尝试序号冲突",
            (Locale::EnUs, "denied") => "Permission denied",
            (Locale::ZhCn, "denied") => "权限不足",
            (Locale::EnUs, "unknown") => "Submission outcome unknown",
            (Locale::ZhCn, "unknown") => "提交结果不确定",
            (Locale::EnUs, _) => "Unavailable",
            (Locale::ZhCn, _) => "不可用",
        };
        let mut text = format!("{label}\n");
        let fields = [
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
                "工作区状态",
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
                    if self.locale == Locale::EnUs { en } else { zh },
                    escape(value)
                ));
            }
        }
        let restoration = &status["progress"]["restoration"];
        if let Some(total) = restoration["total"].as_u64() {
            let attempted = restoration["attempted"].as_u64().expect("attempted shells");
            let started = restoration["started"].as_u64().expect("started shells");
            let failed = restoration["failed"].as_u64().expect("failed shells");
            let unknown = restoration["unknown"].as_u64().expect("unknown shells");
            let pending = attempted - started - failed - unknown;
            text.push_str(&match self.locale {
                Locale::EnUs => format!("Shell restoration: {started}/{total} started; {failed} failed; {unknown} unknown; {pending} pending; {} unattempted\n", total - attempted),
                Locale::ZhCn => format!("shell 恢复：已启动 {started}/{total}；失败 {failed}；未知 {unknown}；进行中 {pending}；未尝试 {}\n", total - attempted),
            });
            if let Some(reason) = restoration["failure"]["error"]
                .as_str()
                .or_else(|| restoration["failure"]["kind"].as_str())
            {
                text.push_str(&format!(
                    "{}: {}\n",
                    if self.locale == Locale::EnUs {
                        "Restoration failure"
                    } else {
                        "恢复失败"
                    },
                    escape(reason)
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
            text.push_str(match (self.locale, wait) {
                (Locale::EnUs, "detached") => "The Server continues in the background.\n",
                (Locale::ZhCn, "detached") => "Server 将继续在后台执行。\n",
                (Locale::EnUs, "cancelled") => {
                    "Waiting cancelled. An accepted operation continues on the Server.\n"
                }
                (Locale::ZhCn, "cancelled") => "已取消等待；Server 继续执行已接受的操作。\n",
                (Locale::EnUs, "timed-out") => {
                    "Waiting timed out. An accepted operation continues on the Server.\n"
                }
                (Locale::ZhCn, "timed-out") => "等待超时；Server 继续执行已接受的操作。\n",
                (Locale::EnUs, "observation-failed") => {
                    "Observation failed. The last known operation state is retained.\n"
                }
                (Locale::ZhCn, "observation-failed") => "观测失败；保留最近已知的操作状态。\n",
                (Locale::EnUs, _) => {
                    "Connection lost. The last observed state is retained below.\n"
                }
                (Locale::ZhCn, _) => "连接中断；保留最近观察到的状态。\n",
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
        if status["resumable"] == false
            && status["reason"]["error"] == "checkout-not-found"
            && status["progress"]["workspace"]["kind"] == "unassociated"
        {
            text.push_str(match self.locale {
                Locale::EnUs => "The original checkout is absent; this operation cannot resume. Inspect `git worktree list` and the retained branch, fix the Git error, then issue a new `orchbee worktree create` request.\n",
                Locale::ZhCn => "原 checkout 已不存在，此操作无法恢复。请检查 `git worktree list` 和保留的分支，解决 Git 错误后重新运行 `orchbee worktree create`。\n",
            });
        }
        if let Some(post) = result.get("postOpen") {
            text.push_str(match (self.locale, post["outcome"].as_str()) {
                (Locale::EnUs, Some("completed")) => "TUI focus completed.\n",
                (Locale::ZhCn, Some("completed")) => "TUI 聚焦已完成。\n",
                (Locale::EnUs, _) => {
                    "TUI focus incomplete; the Worktree result above is retained.\n"
                }
                (Locale::ZhCn, _) => "TUI 聚焦未完成；保留上方工作树结果。\n",
            });
            if let Some(failure) = post["failure"].as_str() {
                text.push_str(&format!(
                    "{}: {}\n",
                    if self.locale == Locale::EnUs {
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

    pub(super) fn cli_worktree_help(self, path: CommandPath) -> crate::cli_text::HelpDetails {
        match (self.locale, path) {
            (Locale::EnUs, CommandPath::WorktreeCreate) => crate::cli_text::HelpDetails::new(
                "orchbee worktree create [--project <target>] [--workspace <target>|--cwd <path>] [--branch <name>] [--base <ref>] [--path <path>] [--label <label>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json] [--client <id>] [--focus|--no-focus]",
                "Windows: create a Git worktree, associate a Workspace, and launch its terminal.\nSource: explicit --client overrides caller; otherwise caller, unique current ready TUI, then CLI cwd only with no caller/Clients. A Workspace source keeps its exact identity; the Server validates its primary directory.\nWorkspace sources may be ungrouped: the server atomically groups source and new Workspace, reusing an existing group. Only Directory sources accept --project; without it the new Workspace is ungrouped. JSON v1 has no Project setup stage.\nWindows paths: absolute, relative or ~ (USERPROFILE, HOMEDRIVE+HOMEPATH, HOME). The final executing CLI resolves its own cwd/home after routing; sender and relay preserve raw operands. No shell expansion.\nDefault branch: worktree/<operation-id>; default base: HEAD.\nDefault path: sibling <repository>-worktrees/<branch-slug>. Existing branches ignore --base.\n--trust-repository grants Git trust only to this operation. It does not change Git configuration.\n--focus switches an existing TUI after opening; default --no-focus. Focus rejects --detach; a focus failure retains the Worktree result (JSON v1).\nTimeout: 1–3600000 ms, default 120000. --detach returns the retained operation immediately.\nCtrl+C stops waiting; status reads the continuing operation. Do not replay an uncertain mutation.\nOptional terminal geometry: --cols <n> --rows <n> --cell-width <n> --cell-height <n> (all four).",
            ),
            (Locale::ZhCn, CommandPath::WorktreeCreate) => crate::cli_text::HelpDetails::new(
                "orchbee worktree create [--project <target>] [--workspace <target>|--cwd <路径>] [--branch <名称>] [--base <引用>] [--path <路径>] [--label <标签>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json] [--client <id>] [--focus|--no-focus]",
                "Windows：创建 Git worktree，关联工作区并启动其终端。\n来源：显式 --client 优先于调用者；否则依次为调用者、唯一当前就绪 TUI、无调用者和客户端时的 CLI cwd。工作区来源始终保留其稳定标识；主目录由服务端验证。\nWorkspace 来源可以未分组；服务端原子创建项目并将来源和新工作区分组，或复用已有分组。仅目录来源可用 --project 指定已有项目；默认创建未分组工作区。JSON v1 无项目准备阶段。\nWindows 路径支持绝对路径、相对路径或 ~（USERPROFILE、HOMEDRIVE+HOMEPATH、HOME）。路由后由最终执行的 CLI 使用自身 cwd／主目录解析，发送端与中继保留原始操作数。不做 shell 展开。\n默认分支：worktree/<操作标识>；默认基准：HEAD。\n默认路径：同级 <仓库名>-worktrees/<分支路径名>。已有分支忽略 --base。\n--trust-repository 仅对此操作授予 Git 信任，不修改 Git 配置。\n--focus 在打开后切换已有 TUI，默认 --no-focus。聚焦不可与 --detach 混用；聚焦失败保留工作树结果（JSON v1）。\n超时：1–3600000 毫秒，默认 120000。--detach 立即返回保留的操作。\nCtrl+C 停止等待；status 可查询继续执行的操作。不要重放结果不确定的变更。\n可选终端尺寸：--cols <n> --rows <n> --cell-width <n> --cell-height <n>（四项全填）。",
            ),
            (Locale::EnUs, CommandPath::WorktreeOpen) => crate::cli_text::HelpDetails::new(
                "orchbee worktree open (--path <path>|--branch <name>) [--project <target>] [--workspace <target>|--cwd <path>] [--label <label>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json] [--client <id>] [--focus|--no-focus]",
                "Windows: inspect an existing checkout and reuse or create its Workspace. No Git checkout is created.\nMain and linked checkouts are supported; ambiguous branch selection requires --path.\nSource/project defaults, absolute/relative/~ paths, terminal geometry and waiting follow worktree create.\n--focus switches an existing TUI after opening; default --no-focus. Focus cannot be detached.\n--label also renames a reused Workspace. Inspect partial launch outcomes in the result.",
            ),
            (Locale::ZhCn, CommandPath::WorktreeOpen) => crate::cli_text::HelpDetails::new(
                "orchbee worktree open (--path <路径>|--branch <名称>) [--project <target>] [--workspace <target>|--cwd <路径>] [--label <标签>] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json] [--client <id>] [--focus|--no-focus]",
                "Windows：检查已有检出目录，复用或创建其工作区；不会新建 Git 检出目录。支持主检出及关联检出；分支选择有歧义时须使用 --path。来源、项目默认值、绝对／相对／~ 路径、终端尺寸和等待方式与 worktree create 一致。\n--focus 在打开后切换已有 TUI；默认 --no-focus，聚焦不可与 --detach 混用。\n--label 同样会重命名复用的工作区；请检查结果中的终端启动状态。",
            ),
            (Locale::EnUs, CommandPath::WorktreeRemove) => crate::cli_text::HelpDetails::new(
                "orchbee worktree remove --workspace <target> --confirm [--force] [--trust-repository] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]",
                "Windows: inspect the linked checkout, dispose its terminals, remove it through Git, then close the Workspace.\nDirty worktrees are refused before closing unless --force explicitly permits discarding changes.\nMain/bare checkouts and checkouts used by another Workspace are refused. Branches are preserved.\nKnown Git failure verifies the checkout and restores default shells in the original panels. Unknown deletion starts no shells. JSON v1 retains separate restoration counts and failures; unknown launches cannot be replayed.\nTimeout defaults to 120000 ms. Ctrl+C stops waiting; it does not undo accepted work.",
            ),
            (Locale::ZhCn, CommandPath::WorktreeRemove) => crate::cli_text::HelpDetails::new(
                "orchbee worktree remove --workspace <target> --confirm [--force] [--trust-repository] [--request-id <id>] [--detach] [--timeout <毫秒>] [--format text|json]",
                "Windows：检查关联检出目录，释放终端，通过 Git 移除目录，最后关闭工作区。脏目录会在关闭前拒绝；--force 明确允许丢弃改动。拒绝主检出、裸仓库及其他工作区占用的目录；保留分支。已知 Git 失败会验证 checkout，并在原面板恢复默认 shell。删除结果未知时不启动 shell。JSON v1 独立保留恢复计数和失败，未知启动不可重放。默认超时 120000 毫秒。Ctrl+C 停止等待，不撤销已接受的操作。",
            ),
            (Locale::EnUs, CommandPath::WorktreeStatus) => crate::cli_text::HelpDetails::new(
                "orchbee worktree status <operation-id> [--format text|json]",
                "Read a retained operation without mutation or session activation. IDs survive CLI disconnection, not Server restart.\nThe result includes attempt, Git effects, Workspace state, shell restoration counts/failure and recovery eligibility.",
            ),
            (Locale::ZhCn, CommandPath::WorktreeStatus) => crate::cli_text::HelpDetails::new(
                "orchbee worktree status <operation-id> [--format text|json]",
                "只读查询保留的操作，不执行变更或激活会话。记录可跨 CLI 断线保留，但不跨 Server 重启。结果包含尝试序号、Git 影响、工作区状态、shell 恢复计数／失败及能否恢复。",
            ),
            (Locale::EnUs, _) => crate::cli_text::HelpDetails::new(
                "orchbee worktree resume <operation-id> --generation <attempt> --confirm [--detach] [--timeout <ms>] [--format text|json]",
                "Explicitly resume a known, resumable failed operation using the attempt from status.\nA changed attempt, Worker incarnation or unproven process settlement is refused.\nGit creation is observed before recovery; Remove may close restored shells before retrying deletion. An already closed Workspace stays closed.\nConfirmation is required because the retained operation can remove user work.",
            ),
            (Locale::ZhCn, _) => crate::cli_text::HelpDetails::new(
                "orchbee worktree resume <operation-id> --generation <尝试序号> --confirm [--detach] [--timeout <毫秒>] [--format text|json]",
                "使用 status 返回的尝试序号，显式恢复已知且可恢复的失败操作。尝试序号、Worker 实例已变化或进程收尾无法证明时拒绝恢复。恢复前先观察 Git 创建结果；Remove 重试删除前可能先关闭已恢复的 shell。已关闭的工作区保持关闭。保留的操作可能删除用户工作，因此必须确认。",
            ),
        }
    }
}

impl super::CliText {
    pub(crate) fn cli_worktree_query(
        self,
        outcome: &orchbee_workbench_protocol::WorkbenchWorktreeQueryOutcome,
    ) -> String {
        use orchbee_workbench_protocol::{
            WorkbenchWorktreeQueryOutcome as O, WorktreeEntryText, WorktreeQueryFailure as F,
        };
        let word = |en, zh| match self.locale {
            Locale::EnUs => en,
            Locale::ZhCn => zh,
        };
        match outcome {
            O::Found {
                source, entries, ..
            } => {
                let mut text = format!(
                    "{}: {}\n{}: {}\n",
                    word("Repository", "仓库"),
                    native_path(source.repository_root.as_str()),
                    word("Source checkout", "来源检出"),
                    native_path(source.checkout_path.as_str())
                );
                for entry in entries {
                    let checkout = &entry.checkout;
                    let WorktreeEntryText::Utf8(path) = &checkout.path else {
                        unreachable!("validated UTF-8 query")
                    };
                    let branch = match &checkout.branch {
                        Some(WorktreeEntryText::Utf8(branch)) => branch.as_str(),
                        _ if checkout.bare => word("bare", "裸仓库"),
                        // This operation needs UTF-8 text operands; opaque path bytes cannot supply them.
                        _ => word("detached HEAD", "分离 HEAD"),
                    };
                    let kind = if entry.linked {
                        word("linked", "关联检出")
                    } else {
                        word("main", "主检出")
                    };
                    text.push_str(&format!(
                        "\n{}  [{}]  {}\n",
                        native_path(path),
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
                        text.push_str(word("  No associated Workspace\n", "  未关联工作区\n"));
                    }
                    for workspace in &entry.workspaces {
                        text.push_str(&format!(
                            "  {}: {} ({})  {}: {}\n",
                            word("Workspace", "Workspace"),
                            escape(workspace.label.as_str()),
                            workspace.workspace_id.as_str(),
                            word("Project", "项目"),
                            workspace
                                .project_id
                                .as_ref()
                                .map(orchbee_workbench_protocol::WorkbenchProjectId::as_str)
                                .unwrap_or(self.cli_ungrouped())
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
                    F::GitFailed {
                        stage: orchbee_workbench_protocol::WorktreeListStage::Probe,
                        ..
                    } => word(
                        "Not a Git repository, or Git does not trust its owner; run `git status` in that directory to see which",
                        "不是 Git 仓库，或 Git 不信任其所有者；在该目录运行 `git status` 查看原因",
                    ),
                    F::GitFailed { .. } => word(
                        "Git could not list the repository's worktrees",
                        "Git 无法列出该仓库的 worktree",
                    ),
                    F::SpawnUnavailable { .. } => {
                        word("Git process owner unavailable", "Git 进程所有者不可用")
                    }
                    F::SpawnFailed { .. } => word("Could not start Git", "无法启动 Git"),
                    F::TooLarge { .. } => word(
                        "Worktree listing exceeds its output limit",
                        "工作树列表超出输出上限",
                    ),
                    F::Timeout {} => word("Worktree query timed out", "工作树查询超时"),
                    F::OwnerIo { .. } => word("Git query I/O failed", "Git 查询 I/O 失败"),
                    F::TransportLoss {} => {
                        word("Connection lost during the query", "查询期间连接中断")
                    }
                    F::MalformedListing {} => word(
                        "Git returned an invalid worktree listing",
                        "Git 返回了无效的工作树列表",
                    ),
                };
                format!("{message}\n")
            }
        }
    }
}

pub(super) const HELP_WORKTREE_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee worktree list [--workspace <target>|--cwd <path>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --directory <directory-id> [--format text|json]",
        "Windows: list Git checkouts and associated Workspaces. With no source selector,\n--client selects an exact ready TUI; otherwise use the validated caller, then a unique\ncurrent ready TUI, then CLI cwd only when neither exists. A Workspace source keeps its exact identity and must have a primary directory. No Project is required for a directory query. Invalid caller identity\nnever falls back. --trust-repository trusts this repository for this query only.\nThe query is read-only, does not activate a session, and has a 5-second total budget.\nJSON source queries emit orchbee.worktree.query; failed Server results keep typed reasons.\nThe globally stable --directory selector keeps orchbee.worktree.list and its failure schema;\nthese selectors cannot be combined with --workspace, --cwd, --client or --trust-repository.\nWorkspace targets accept exact IDs, @workspace:0x<hex> or exact labels; ambiguity is rejected.\nBranches keep refs/heads/ prefixes; all-zero HEAD marks an unborn repository.\nWindows --cwd accepts absolute, relative and ~ paths; the final executing CLI resolves its own cwd/home after routing. Home precedence: USERPROFILE, HOMEDRIVE+HOMEPATH, HOME.",
    );

pub(super) const HELP_WORKTREE_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee worktree list [--workspace <target>|--cwd <路径>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --directory <directory-id> [--format text|json]",
        "Windows：列出 Git 检出目录及关联工作区。省略来源选择器时，已验证的调用者终端\n优先，其次为唯一当前就绪 TUI，均无时才使用 CLI cwd。--client 选择精确就绪 TUI，\n覆盖隐式调用者。工作区来源保留其精确标识且必须有主目录。目录查询无需项目。无效调用者身份\n绝不回退。--trust-repository 仅在本次查询中信任该仓库。\n查询只读、不激活会话，总预算 5 秒。来源查询 JSON 使用 orchbee.worktree.query，\nServer 失败结果保留类型化原因。全局稳定 --directory 选择器保留 orchbee.worktree.list\n及其失败 Schema；不可混用 --workspace、--cwd、--client 或 --trust-repository。\nWorkspace 目标支持精确 ID、@workspace:0x<hex> 或精确标签；拒绝歧义。\n分支保留 refs/heads/ 前缀；全零 HEAD 表示未出生仓库。\nWindows --cwd 接受绝对、相对和 ~ 路径；路由后由最终执行的 CLI 使用自身 cwd／主目录解析。主目录优先级：USERPROFILE、HOMEDRIVE+HOMEPATH、HOME。",
    );

impl super::CliText {
    /// `worktree list` failure diagnostics, keyed by the details the typed
    /// outcome mapping produced. Every line states what the listing observed.
    pub(super) fn cli_worktree_failure(self, failure: &Failure) -> String {
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
                match self.locale {
                    Locale::EnUs => {
                        format!("No directory with ID {target} is registered to the project.\n")
                    }
                    Locale::ZhCn => format!("该项目未注册 ID 为 {target} 的目录。\n"),
                }
            }
            FailureKind::Denied => match self.locale {
                Locale::EnUs => "The local session denied the worktree listing.\n".to_owned(),
                Locale::ZhCn => "本地会话拒绝了该 worktree 列举。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => {
                let reason = detail("reason");
                match (self.locale, reason) {
                    (Locale::EnUs, "job-dead") => {
                        "The directory's worktree listing job is no longer alive.\n".to_owned()
                    }
                    (Locale::ZhCn, "job-dead") => {
                        "该目录的 worktree 列举作业已不存在。\n".to_owned()
                    }
                    (Locale::EnUs, "job-unknown") => {
                        "The directory's worktree listing job is unknown to the Worker.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, "job-unknown") => {
                        "Worker 不认识该目录的 worktree 列举作业。\n".to_owned()
                    }
                    (Locale::EnUs, "worker-unavailable") => {
                        "The directory's Worker is not available for the worktree listing right now.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, "worker-unavailable") => {
                        "该目录的 Worker 当前无法进行 worktree 列举。\n".to_owned()
                    }
                    (Locale::EnUs, _) => {
                        "The local session could not run the worktree listing right now.\n".to_owned()
                    }
                    (Locale::ZhCn, _) => "本地会话当前无法执行 worktree 列举。\n".to_owned(),
                }
            }
            FailureKind::CheckFailed => {
                let stage = detail("stage");
                match (self.locale, stage) {
                    (Locale::EnUs, "parse") => {
                        "The directory's Git worktree listing was malformed.\n".to_owned()
                    }
                    (Locale::ZhCn, "parse") => {
                        "该目录的 Git worktree 列表输出格式无效。\n".to_owned()
                    }
                    (Locale::EnUs, stage) => {
                        format!("The directory's Git worktree {stage} stage failed.\n")
                    }
                    (Locale::ZhCn, stage) => {
                        format!("该目录的 Git worktree {stage} 阶段失败。\n")
                    }
                }
            }
            FailureKind::Timeout => match self.locale {
                Locale::EnUs => {
                    "The worktree listing did not complete within the time limit.\n".to_owned()
                }
                Locale::ZhCn => "worktree 列举未在时限内完成。\n".to_owned(),
            },
            FailureKind::OutputTooLarge => {
                let origin = detail("origin");
                match self.locale {
                    Locale::EnUs => {
                        format!("The worktree listing exceeded its output bound ({origin}).\n")
                    }
                    Locale::ZhCn => format!("worktree 列举超出其输出上限（{origin}）。\n"),
                }
            }
            FailureKind::TransportLoss => match self.locale {
                Locale::EnUs => {
                    "The connection to the local session was lost during the worktree listing.\n"
                        .to_owned()
                }
                Locale::ZhCn => "与本地会话的连接在 worktree 列举期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self.locale {
                Locale::EnUs => {
                    "The local session answered the worktree listing with an invalid protocol response.\n"
                        .to_owned()
                }
                Locale::ZhCn => "本地会话对 worktree 列举返回了无效的协议响应。\n".to_owned(),
            },
            _ => match self.locale {
                Locale::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Locale::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }
}
