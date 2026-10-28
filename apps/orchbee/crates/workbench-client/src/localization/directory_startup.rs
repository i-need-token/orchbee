use super::Locale;

impl Locale {
    pub(crate) fn directory_startup_failure(
        self,
        workspace: Option<&str>,
        unknown: Option<&str>,
    ) -> String {
        self.workspace_creation_failure(true, None, workspace, unknown)
    }
    pub(crate) fn quick_workspace_failure(
        self,
        project: Option<&str>,
        workspace: Option<&str>,
        unknown: Option<&str>,
    ) -> String {
        self.workspace_creation_failure(false, project, workspace, unknown)
    }
    fn workspace_creation_failure(
        self,
        initial: bool,
        project: Option<&str>,
        workspace: Option<&str>,
        unknown: Option<&str>,
    ) -> String {
        let mut text = match (self, initial) {
            (Self::EnUs, true) => "Initial workspace creation did not finish.\n",
            (Self::ZhCn, true) => "首次工作区创建未完成。\n",
            (Self::EnUs, false) => "Workspace creation did not finish.\n",
            (Self::ZhCn, false) => "工作区创建未完成。\n",
        }
        .to_owned();
        if let Some(id) = project {
            let label = match (self, initial) {
                (Self::EnUs, true) => "Created Project",
                (Self::ZhCn, true) => "已创建项目",
                (Self::EnUs, false) => "Project",
                (Self::ZhCn, false) => "项目",
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
        text.push_str(match (self, initial) {
            (Self::EnUs, true) => "Inspect retained state with 'orchbee workspace list'.\n",
            (Self::ZhCn, true) => "运行 ‘orchbee workspace list’ 查看保留的数据。\n",
            (Self::EnUs, false) => {
                "Inspect retained state with 'orchbee project list' and 'orchbee workspace list'.\n"
            }
            (Self::ZhCn, false) => {
                "运行 ‘orchbee project list’ 和 ‘orchbee workspace list’ 查看保留的数据。\n"
            }
        });
        text
    }
}
