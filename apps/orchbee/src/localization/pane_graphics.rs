use super::Locale;
impl Locale {
    pub(crate) const fn pane_graphics_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee panel graphics set PANEL --spec JSON|@FILE|- [--presentation ID] [--timeout MS] [--format text|json]\norchbee panel graphics clear PANEL [--spec '{\"layerId\":\"primary\"}'] [same options]\norchbee panel graphics info PANEL [--client ID] [same options]\norchbee panel graphics stream PANEL [--spec '{\"layerId\":\"primary\",\"zIndex\":0}'] [same options]\n\nSet accepts PNG or a closed JSON object with image and optional layerId/zIndex. Image has format, width, height, dataBase64 and placement {viewportColumn,viewportRow,gridColumns,gridRows}. @FILE reads a bounded regular file; - reads noninteractive stdin. Set input is at most 512 KiB (768 KiB JSON). Stream reads an 8 KiB JSON header then dataLength raw bytes (at most 16 MiB): format, width, height, dataLength and optional sequence, revision, placement, damage. Full-width sequence/revision use decimal strings. Stream default is unbounded duration with 30 seconds per frame; other commands default to 5 seconds. Static layers persist; EOF or Ctrl+C releases this connection's stream. Unknown writes are reported and never replayed. Actual visibility and draw facts require a connected Client.\n"
            }
            Self::ZhCn => {
                "orchbee panel graphics set PANEL --spec JSON|@FILE|- [--presentation ID] [--timeout MS] [--format text|json]\norchbee panel graphics clear PANEL [--spec '{\"layerId\":\"primary\"}'] [同上参数]\norchbee panel graphics info PANEL [--client ID] [同上参数]\norchbee panel graphics stream PANEL [--spec '{\"layerId\":\"primary\",\"zIndex\":0}'] [同上参数]\n\nSet 接受 PNG 或含 image 及可选 layerId/zIndex 的封闭 JSON 对象。Image 含 format、width、height、dataBase64 和 placement {viewportColumn,viewportRow,gridColumns,gridRows}。@FILE 有界读取普通文件；- 读取非交互 stdin。Set 输入最多 512 KiB（JSON 768 KiB）。Stream 读取最多 8 KiB JSON 头，再读取 dataLength 原始字节（最多 16 MiB）：format、width、height、dataLength，以及可选 sequence、revision、placement、damage。完整宽度序号/版本使用十进制字符串。Stream 默认持续运行，单帧预算 30 秒；其余命令默认 5 秒。静态图层保留；EOF 或 Ctrl+C 释放本连接的流。写入结果未知时明确报告且不重放。真实可见性与绘制事实需要已连接 Client。\n"
            }
        }
    }
    pub(crate) const fn pane_graphics_status(self, status: u8) -> &'static str {
        match (self, status) {
            (Self::EnUs, 0) => "Graphics operation completed.\n",
            (Self::ZhCn, 0) => "图形操作已完成。\n",
            (Self::EnUs, 8) => "Graphics submission or cleanup outcome is unknown.\n",
            (Self::ZhCn, 8) => "图形提交或清理结果未知。\n",
            (Self::EnUs, _) => "Graphics operation was refused or interrupted.\n",
            (Self::ZhCn, _) => "图形操作被拒绝或中断。\n",
        }
    }
}
