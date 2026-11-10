//! Windows installation diagnostics, separate from language-neutral receipt reasons.
use super::Locale;
impl super::CliText {
    pub(crate) fn cli_update_message(self, code: &str) -> &'static str {
        let pair = match code {
            "complete" => (
                "Update installed. Open a new terminal or sign in again to use it. Existing sessions keep their current version.",
                "更新已安装。请新开终端或重新登录后使用；现有会话继续运行原版本。",
            ),
            "channel-saved" => (
                "Channel preference saved. It remains saved if the update fails.",
                "频道偏好已保存；更新失败时也会保留该偏好。",
            ),
            "non-release-build" => (
                "This is a development or test build, so it does not update the release installation. Nothing was changed.",
                "当前是开发或测试构建，不会更新正式版安装，未做任何改动。",
            ),
            "configuration" => (
                "Cannot read or save configuration. Run orchbee config check.",
                "无法读取或保存配置。请运行 orchbee config check。",
            ),
            "source-unconfigured" => (
                "Set ORCHBEE_UPDATE_MANIFEST to an Orchbee HTTPS release manifest or an absolute local manifest path, then retry orchbee update.",
                "请将 ORCHBEE_UPDATE_MANIFEST 设置为 Orchbee HTTPS 发布清单或本地清单的绝对路径，再重试 orchbee update。",
            ),
            "source-invalid" => (
                "Invalid release source. ORCHBEE_UPDATE_MANIFEST must be an HTTPS URL without a query or an absolute local manifest path.",
                "发布源无效。ORCHBEE_UPDATE_MANIFEST 必须是不带查询参数的 HTTPS URL，或本地清单的绝对路径。",
            ),
            "signing-keys-unavailable" => (
                "This build trusts no release signing key, so it refuses every update. Nothing was changed. Install a newer Orchbee manually.",
                "当前构建没有可信的发布签名密钥，会拒绝所有更新，未做任何改动。请手动安装更新版本的 Orchbee。",
            ),
            "signature-untrusted" => (
                "The release manifest is not signed by a key this Orchbee trusts. Nothing was installed. If the signing key changed, install a newer Orchbee manually.",
                "发布清单不是由当前 Orchbee 信任的密钥签名的，未安装任何内容。如果签名密钥已更换，请手动安装更新版本的 Orchbee。",
            ),
            "signature-invalid" => (
                "The release signature (release.json.sig beside the manifest) is missing, malformed, or does not match the manifest. Nothing was installed.",
                "发布签名（清单旁的 release.json.sig）缺失、格式错误或与清单不符，未安装任何内容。",
            ),
            "local-manifest-read" => (
                "Cannot read the local release manifest. Check the file path and permissions in ORCHBEE_UPDATE_MANIFEST, then retry orchbee update.",
                "无法读取本地发布清单。请检查 ORCHBEE_UPDATE_MANIFEST 中的文件路径和读取权限，再重试 orchbee update。",
            ),
            "manifest-invalid" => (
                "Release manifest is invalid, incomplete, or for another channel/platform.",
                "发布清单无效、不完整，或频道/平台不匹配。",
            ),
            "download" => (
                "Release acquisition failed; the installed version is unchanged.",
                "无法获取发布文件；已安装版本未改变。",
            ),
            "download-unavailable" => (
                "HTTPS updates require system curl 8.4 or newer for bounded downloads.",
                "HTTPS 更新需要系统 curl 8.4 或更新版本以限制下载大小。",
            ),
            "checksum" => (
                "Release file size or SHA-256 verification failed.",
                "发布文件大小或 SHA-256 校验失败。",
            ),
            "storage" => (
                "Cannot stage or publish the release safely.",
                "无法安全暂存或发布此版本。",
            ),
            "install-location" => (
                "Invalid ORCHBEE_INSTALL_DIR or LOCALAPPDATA installation location.",
                "ORCHBEE_INSTALL_DIR 或 LOCALAPPDATA 安装位置无效。",
            ),
            "busy" => (
                "Another update owns this installation. Retry after it finishes.",
                "另一个更新正在使用此安装目录，请稍后重试。",
            ),
            "cohort-invalid" => (
                "The downloaded installation is incompatible or damaged.",
                "下载的安装文件不兼容或已损坏。",
            ),
            "smoke-failed" => (
                "The verified release failed its version startup check.",
                "已校验版本未通过版本启动检查。",
            ),
            "activation" => (
                "Release published, but user PATH activation failed. Retry orchbee update.",
                "版本已发布，但用户 PATH 激活失败。请重试 orchbee update。",
            ),
            "activation-unknown" => (
                "Release published; user PATH activation was not confirmed. Retry orchbee update to reconcile it.",
                "版本已发布，但用户 PATH 激活结果未确认。请重试 orchbee update 以核对并完成。",
            ),
            "cancelled" => (
                "Update cancelled. Any published release remains at the reported path.",
                "更新已取消；已发布版本保留在回执中的路径。",
            ),
            "timeout" => ("Update timed out.", "更新超时。"),
            "physical-unknown" => (
                "Update child-process settlement is unconfirmed.",
                "更新子进程的结束状态未确认。",
            ),
            "cleanup-pending" => (
                "Staging cleanup requires recovery: ",
                "暂存目录需恢复清理：",
            ),
            _ => ("Update could not finish.", "更新未能完成。"),
        };
        match self.locale {
            Locale::EnUs => pair.0,
            Locale::ZhCn => pair.1,
        }
    }
}
