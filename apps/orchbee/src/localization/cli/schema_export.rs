use super::Locale;
use crate::cli::escape::diagnostic_token;

impl Locale {
    #[cfg(windows)]
    pub(crate) fn schema_export_failure(
        self,
        stage: &str,
        path: &str,
        residue: Option<&str>,
    ) -> String {
        let action = match (self, stage) {
            (Self::EnUs, "destination") => "checking the destination",
            (Self::EnUs, "create-temporary") => "creating a temporary file",
            (Self::EnUs, "write-temporary") => "writing the temporary file",
            (Self::EnUs, "sync-temporary") => "flushing the temporary file",
            (Self::EnUs, _) => "replacing the destination",
            (Self::ZhCn, "destination") => "检查目标文件",
            (Self::ZhCn, "create-temporary") => "创建临时文件",
            (Self::ZhCn, "write-temporary") => "写入临时文件",
            (Self::ZhCn, "sync-temporary") => "刷写临时文件",
            (Self::ZhCn, _) => "替换目标文件",
        };
        let path = diagnostic_token(path);
        let mut text = match self {
            Self::EnUs => format!("Could not export schema to {path} while {action}.\n"),
            Self::ZhCn => format!("导出 schema 到 {path} 失败：无法{action}。\n"),
        };
        if let Some(residue) = residue {
            let residue = diagnostic_token(residue);
            text.push_str(&match self {
                Self::EnUs => format!("Temporary file could not be removed: {residue}.\n"),
                Self::ZhCn => format!("临时文件未能删除：{residue}。\n"),
            });
        }
        text.push_str(match self {
            Self::EnUs => "Run 'orchbee help api schema' for usage.\n",
            Self::ZhCn => "用法请运行 'orchbee help api schema'。\n",
        });
        text
    }
}
