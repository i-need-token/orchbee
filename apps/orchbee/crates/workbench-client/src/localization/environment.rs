use super::Locale;
use crate::environment_marker::MarkedEnvironment;

impl Locale {
    /// The sidebar marker's word for a non-release build.
    pub(crate) const fn environment_label(self, environment: MarkedEnvironment) -> &'static str {
        match (self, environment) {
            (Self::EnUs, MarkedEnvironment::Development) => "Dev",
            (Self::ZhCn, MarkedEnvironment::Development) => "开发版",
            (Self::EnUs, MarkedEnvironment::Test) => "Test",
            (Self::ZhCn, MarkedEnvironment::Test) => "测试版",
        }
    }

    /// The marker's tooltip: the build and the checkout it came from, whose name the marker shows
    /// only when it fits whole.
    pub(crate) fn environment_checkout(
        self,
        environment: MarkedEnvironment,
        checkout: &str,
    ) -> String {
        match (self, environment) {
            (Self::EnUs, MarkedEnvironment::Development) => {
                format!("Development build from the {checkout} checkout")
            }
            (Self::EnUs, MarkedEnvironment::Test) => {
                format!("Test build from the {checkout} checkout")
            }
            (Self::ZhCn, MarkedEnvironment::Development) => {
                format!("开发版，构建自检出目录 {checkout}")
            }
            (Self::ZhCn, MarkedEnvironment::Test) => format!("测试版，构建自检出目录 {checkout}"),
        }
    }
}
