//! Formats schema-export path and failure diagnostics without creating or publishing files.
use super::Locale;
use crate::cli::escape::diagnostic_token;

impl super::CliText {
    pub(crate) fn schema_export_failure(
        self,
        stage: &str,
        path: &str,
        residue: Option<&str>,
    ) -> String {
        let action = match (self.locale, stage) {
            (Locale::EnUs, "destination") => "checking the destination",
            (Locale::EnUs, "create-temporary") => "creating a temporary file",
            (Locale::EnUs, "write-temporary") => "writing the temporary file",
            (Locale::EnUs, "sync-temporary") => "flushing the temporary file",
            (Locale::EnUs, _) => "replacing the destination",
            (Locale::ZhCn, "destination") => "检查目标文件",
            (Locale::ZhCn, "create-temporary") => "创建临时文件",
            (Locale::ZhCn, "write-temporary") => "写入临时文件",
            (Locale::ZhCn, "sync-temporary") => "刷写临时文件",
            (Locale::ZhCn, _) => "替换目标文件",
        };
        let path = diagnostic_token(path);
        let mut text = match self.locale {
            Locale::EnUs => format!("Could not export schema to {path} while {action}.\n"),
            Locale::ZhCn => format!("导出 schema 到 {path} 失败：无法{action}。\n"),
        };
        if let Some(residue) = residue {
            let residue = diagnostic_token(residue);
            text.push_str(&match self.locale {
                Locale::EnUs => format!("Temporary file could not be removed: {residue}.\n"),
                Locale::ZhCn => format!("临时文件未能删除：{residue}。\n"),
            });
        }
        text.push_str(match self.locale {
            Locale::EnUs => "Run 'orchbee help api schema' for usage.\n",
            Locale::ZhCn => "用法请运行 'orchbee help api schema'。\n",
        });
        text
    }
}
