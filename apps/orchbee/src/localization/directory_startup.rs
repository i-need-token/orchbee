use super::Locale;

impl Locale {
    pub(crate) fn directory_startup_failure(
        self,
        project: Option<&str>,
        workspace: Option<&str>,
        unknown: Option<&str>,
    ) -> String {
        let mut text = match self {
            Self::EnUs => "Initial workspace creation did not finish.\n",
            Self::ZhCn => "首次工作区创建未完成。\n",
        }
        .to_owned();
        if let Some(id) = project {
            let label = match self {
                Self::EnUs => "Created Project",
                Self::ZhCn => "已创建项目",
            };
            text.push_str(&format!("{label}: {id}\n"));
        }
        if let Some(id) = workspace {
            let label = match self {
                Self::EnUs => "Created Workspace",
                Self::ZhCn => "已创建工作区",
            };
            text.push_str(&format!("{label}: {id}\n"));
        }
        if let Some(id) = unknown {
            let label = match self {
                Self::EnUs => "Unconfirmed request",
                Self::ZhCn => "结果不明的请求",
            };
            text.push_str(&format!("{label}: {id}\n"));
        }
        text.push_str(match self {
            Self::EnUs => {
                "Inspect retained state with 'orchbee project list' and 'orchbee workspace list'.\n"
            }
            Self::ZhCn => {
                "运行 ‘orchbee project list’ 和 ‘orchbee workspace list’ 查看保留的数据。\n"
            }
        });
        text
    }
}
