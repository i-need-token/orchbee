use super::Locale;
impl Locale {
    pub(crate) fn pane_graphics_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::PanelGraphicsSet => (
                "orchbee panel graphics set <panel> --spec <json|@file|-> [--presentation <id>] [--timeout <ms>] [--format text|json]",
                "Display PNG data or a JSON object containing image and optional layerId/zIndex. The image fields are format, width, height, dataBase64 and placement {viewportColumn,viewportRow,gridColumns,gridRows}. @file reads a regular file; - reads noninteractive stdin. Input limits: 512 KiB for PNG, 768 KiB for JSON. Layers persist until cleared. Default timeout: 5000 ms.\n\nRequires an existing Site. Actual visibility and drawing status require a connected client. An unknown write is reported and never replayed automatically.",
                "显示 PNG 数据，或含 image 及可选 layerId/zIndex 的 JSON 对象。image 字段为 format、width、height、dataBase64 和 placement {viewportColumn,viewportRow,gridColumns,gridRows}。@file 读取普通文件，- 读取非交互标准输入。输入上限：PNG 512 KiB，JSON 768 KiB。图层保留至显式清除。默认时限 5000 毫秒。\n\n需要已运行的 Site。实际可见性和绘制状态需要已连接的客户端；写入结果未知时会明确报告，不自动重放。",
            ),
            C::PanelGraphicsClear => (
                "orchbee panel graphics clear <panel> [--spec <json>] [--presentation <id>] [--timeout <ms>] [--format text|json]",
                "Clear a graphics layer. --spec can select a layer with {\"layerId\":\"primary\"}. Default timeout: 5000 ms.\n\nRequires an existing Site. Actual visibility and drawing status require a connected client. An unknown write is reported and never replayed automatically.",
                "清除图像图层。--spec 可使用 {\"layerId\":\"primary\"} 指定图层。默认时限 5000 毫秒。\n\n需要已运行的 Site。实际可见性和绘制状态需要已连接的客户端；写入结果未知时会明确报告，不自动重放。",
            ),
            C::PanelGraphicsInfo => (
                "orchbee panel graphics info <panel> [--client <id>] [--presentation <id>] [--timeout <ms>] [--format text|json]",
                "Read graphics state without changing layers. A connected client is required to report actual visibility and drawing status. Default timeout: 5000 ms. Requires an existing Site.",
                "读取图像状态，不改变图层。实际可见性和绘制状态需要已连接的客户端。默认时限 5000 毫秒；需要已运行的 Site。",
            ),
            C::PanelGraphicsStream => (
                "orchbee panel graphics stream <panel> [--spec <json>] [--presentation <id>] [--timeout <ms>] [--format text|json]",
                "Read frames from stdin: an 8 KiB maximum JSON header followed by dataLength raw bytes, up to 16 MiB. Required header fields: format, width, height, dataLength. Optional fields: sequence, revision, placement, damage. Use decimal strings for full-width sequence/revision values. --spec can select layerId and zIndex.\n\nDefault: no overall deadline, with 30 seconds per frame. EOF or Ctrl+C releases this connection's stream. Requires an existing Site. Actual visibility and drawing status require a connected client. An unknown write is reported and never replayed automatically.",
                "从标准输入读取帧：最大 8 KiB 的 JSON 头，后接 dataLength 个原始字节，最多 16 MiB。头部必填 format、width、height、dataLength；可选 sequence、revision、placement、damage。完整宽度的 sequence/revision 使用十进制字符串。--spec 可指定 layerId 和 zIndex。\n\n默认没有总时限，每帧时限 30 秒。EOF 或 Ctrl+C 释放本连接的图像流。需要已运行的 Site。实际可见性和绘制状态需要已连接的客户端；写入结果未知时会明确报告，不自动重放。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::localization::HelpDetails::new(
            usage,
            match self {
                Self::EnUs => english,
                Self::ZhCn => chinese,
            },
        )
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
