//! Windows install and update diagnostics, separate from language-neutral receipt reasons.
use super::Locale;
impl super::CliText {
    pub(crate) fn cli_update_message(self, code: &str) -> &'static str {
        let pair = match code {
            "updated-to" => ("Updated to ", "已更新到 "),
            "complete" => (
                "Running terminals and Agents keep their version; open Orchbee again to use the new one.",
                "运行中的终端和智能体保持原版本；重新打开 Orchbee 即可使用新版本。",
            ),
            "up-to-date" => ("Up to date: ", "已是最新版本："),
            "channel-saved" => (
                "Channel preference saved. It remains saved if the update fails.",
                "频道偏好已保存；更新失败时也会保留该偏好。",
            ),
            "install-complete" => ("Installed and active: ", "已安装并启用："),
            "install-reused" => ("Already installed, now active: ", "已安装，现为当前版本："),
            "path-unavailable" => (
                "PATH not changed: this build has no PATH step, so it ran as if --no-path were given.",
                "未修改 PATH：此构建尚无 PATH 步骤，按 --no-path 处理。",
            ),
            "path-not-requested" => (
                "PATH not changed (--no-path).",
                "未修改 PATH（--no-path）。",
            ),
            "path-not-reached" => ("The PATH step did not run to the end.", "PATH 步骤未完成。"),
            "path-added" => ("Added to your user Path: ", "已添加到用户 Path："),
            "path-present" => ("Already on your user Path: ", "已在用户 Path 中："),
            "path-replaced" => (
                "Replaced old Orchbee entries in your user Path with: ",
                "已将用户 Path 中旧的 Orchbee 条目替换为：",
            ),
            "path-new-terminal" => (
                "Open a new terminal to use orchbee; if it is not found there, sign out and back in.",
                "请打开新终端使用 orchbee；如果仍找不到该命令，请注销后重新登录。",
            ),
            "path-launcher" => (
                "The PATH step could not place the launcher in the bin folder of the installation, so this version was not activated. Run orchbee install --no-path to install without the PATH step.",
                "PATH 步骤无法把启动器放入安装目录的 bin 文件夹，因此未启用此版本。如需不经 PATH 步骤安装，请运行 orchbee install --no-path。",
            ),
            "path-variable" => (
                "The PATH step could not update the user Path (HKCU\\Environment): it is not a text value, would grow past 32,760 characters, or could not be written, so this version was not activated. Run orchbee install --no-path to install without the PATH step.",
                "PATH 步骤无法更新用户 Path（HKCU\\Environment）：它不是文本值、会超过 32,760 个字符或无法写入，因此未启用此版本。如需不经 PATH 步骤安装，请运行 orchbee install --no-path。",
            ),
            "development-build" => (
                "This is a development build, so it does not install. Nothing was changed. Use a release or test package.",
                "当前是开发构建，不能安装，未做任何改动。请使用正式版或测试包。",
            ),
            "not-release-directory" => (
                "This Orchbee is not running from a signed release directory (release.json and release.json.sig beside it), so it cannot install itself. Nothing was changed.",
                "当前 Orchbee 不是从已签名的发布目录（旁边有 release.json 和 release.json.sig）运行的，无法安装自身，未做任何改动。",
            ),
            "version-conflict" => (
                "The installation already holds this version with different contents. Nothing was overwritten.",
                "安装目录中已有同一版本但内容不同，未覆盖任何内容。",
            ),
            "downgrade-refused" => (
                "A newer version is active, so this older release was not activated. Pass --allow-downgrade to orchbee install to allow it.",
                "当前启用的是更新的版本，因此未启用这个旧版本。如需安装旧版本，请给 orchbee install 加 --allow-downgrade。",
            ),
            "other-architecture" => (
                "The installation holds Orchbee for another processor architecture. Nothing was changed.",
                "安装目录中是另一种处理器架构的 Orchbee，未做任何改动。",
            ),
            "non-release-build" => (
                "This is a development build, so it does not update. Nothing was changed. Use a release or test package.",
                "当前是开发构建，不能更新，未做任何改动。请使用正式版或测试包。",
            ),
            "configuration" => (
                "Cannot read or save configuration. Run orchbee config check.",
                "无法读取或保存配置。请运行 orchbee config check。",
            ),
            "source-unconfigured" => (
                "This build has no update feed of its own. Set ORCHBEE_UPDATE_FEED to an Orchbee HTTPS update feed or an absolute local feed path, then retry orchbee update.",
                "此构建没有自带的更新源。请将 ORCHBEE_UPDATE_FEED 设置为 Orchbee HTTPS 更新源或本地更新源文件的绝对路径，再重试 orchbee update。",
            ),
            "source-invalid" => (
                "Invalid update feed. ORCHBEE_UPDATE_FEED must be an HTTPS URL without a query or an absolute local feed path.",
                "更新源无效。ORCHBEE_UPDATE_FEED 必须是不带查询参数的 HTTPS URL，或本地更新源文件的绝对路径。",
            ),
            "signing-keys-unavailable" => (
                "This build trusts no release signing key, so it refuses every install and update. Nothing was changed. Install a newer Orchbee manually.",
                "当前构建没有可信的发布签名密钥，会拒绝所有安装和更新，未做任何改动。请手动安装更新版本的 Orchbee。",
            ),
            "signature-untrusted" => (
                "The update feed or release manifest is not signed by a key this Orchbee trusts. Nothing was installed. If the signing key changed, install a newer Orchbee manually.",
                "更新源或发布清单不是由当前 Orchbee 信任的密钥签名的，未安装任何内容。如果签名密钥已更换，请手动安装更新版本的 Orchbee。",
            ),
            "signature-invalid" => (
                "The signature (the .sig file beside the update feed or release manifest) is missing, malformed, or does not match. Nothing was installed.",
                "签名（更新源或发布清单旁的 .sig 文件）缺失、格式错误或不匹配，未安装任何内容。",
            ),
            "local-feed-read" => (
                "Cannot read the local update feed. Check the file path and permissions in ORCHBEE_UPDATE_FEED, then retry orchbee update.",
                "无法读取本地更新源。请检查 ORCHBEE_UPDATE_FEED 中的文件路径和读取权限，再重试 orchbee update。",
            ),
            "manifest-invalid" => (
                "The update feed or release is invalid, incomplete, or for another channel/platform.",
                "更新源或发布内容无效、不完整，或频道/平台不匹配。",
            ),
            "download" => (
                "Release acquisition failed; the installed version is unchanged.",
                "无法获取发布文件；已安装版本未改变。",
            ),
            "download-unavailable" => (
                "Updates need the system tar and, over HTTPS, system curl 8.4 or newer.",
                "更新需要系统 tar；通过 HTTPS 更新还需要系统 curl 8.4 或更新版本。",
            ),
            "checksum" => (
                "Release file size or SHA-256 verification failed.",
                "发布文件大小或 SHA-256 校验失败。",
            ),
            "storage" => (
                "Cannot unpack, stage, or publish the release safely.",
                "无法安全解包、暂存或发布此版本。",
            ),
            "install-location" => (
                "Invalid ORCHBEE_INSTALL_DIR or LOCALAPPDATA installation location.",
                "ORCHBEE_INSTALL_DIR 或 LOCALAPPDATA 安装位置无效。",
            ),
            "busy" => (
                "Another install or update owns this installation. Retry after it finishes.",
                "另一个安装或更新正在使用此安装目录，请稍后重试。",
            ),
            "cohort-invalid" => (
                "The release files are incompatible or damaged.",
                "发布文件不兼容或已损坏。",
            ),
            "smoke-failed" => (
                "The verified release failed its version startup check.",
                "已校验版本未通过版本启动检查。",
            ),
            "activation" => (
                "Release published, but it could not be made active. The active version is unchanged. Retry the command.",
                "版本已发布，但未能启用，当前版本未改变。请重试该命令。",
            ),
            "activation-unknown" => (
                "Release published; whether it became active was not confirmed. Retry the command to reconcile it.",
                "版本已发布，但是否已启用未确认。请重试该命令以核对并完成。",
            ),
            "cancelled" => (
                "Cancelled. Any published release remains at the reported path.",
                "已取消；已发布版本保留在回执中的路径。",
            ),
            "timeout" => ("Timed out.", "已超时。"),
            "physical-unknown" => (
                "Child-process settlement is unconfirmed.",
                "子进程的结束状态未确认。",
            ),
            "cleanup-pending" => (
                "Staging cleanup requires recovery: ",
                "暂存目录需恢复清理：",
            ),
            "storage-owner" => (
                "its owner is an account other than you, SYSTEM or Administrators.",
                "其所有者不是你、SYSTEM 或 Administrators。",
            ),
            "storage-access" => (
                "its permissions let in an account other than you, SYSTEM or Administrators.",
                "其权限允许你、SYSTEM 和 Administrators 以外的用户访问。",
            ),
            "storage-link" => ("it is a link or junction.", "它是链接或联接点。"),
            "storage-system" => (
                "Windows refused it with error ",
                "Windows 拒绝了操作，错误代码 ",
            ),
            "storage-unknown" => ("it could not be used.", "无法使用它。"),
            _ => ("Could not finish.", "未能完成。"),
        };
        match self.locale {
            Locale::EnUs => pair.0,
            Locale::ZhCn => pair.1,
        }
    }

    /// The one line `update` writes on standard error before it downloads the archive of `version`,
    /// whose signed size is `bytes`, given in whole megabytes (10^6 bytes), at least 1.
    pub(crate) fn cli_update_downloading(self, version: &str, bytes: u64) -> String {
        let megabytes = (bytes.saturating_add(500_000) / 1_000_000).max(1);
        match self.locale {
            Locale::EnUs => format!("Downloading Orchbee {version} ({megabytes} MB)…\n"),
            Locale::ZhCn => format!("正在下载 Orchbee {version}（{megabytes} MB）…\n"),
        }
    }

    /// The line after a `storage` diagnostic: the folder or file the store refused or failed on,
    /// then the reason `key` names, and the system error code for `storage-system`.
    pub(crate) fn cli_storage_cause(self, path: &str, key: &str, code: Option<i32>) -> String {
        let reason = self.cli_update_message(key);
        let code = code.map_or_else(String::new, |code| match self.locale {
            Locale::EnUs => format!("{code}."),
            Locale::ZhCn => format!("{code}。"),
        });
        match self.locale {
            Locale::EnUs => format!("{path}: {reason}{code}"),
            Locale::ZhCn => format!("{path}：{reason}{code}"),
        }
    }
}
