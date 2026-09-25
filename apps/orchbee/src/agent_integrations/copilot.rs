//! Fixed provider vectors shared by inspection, plans, and effects. Windows
//! uses the verified Copilot 1.0.88 plugin contract; Unix retains its existing
//! legacy resource-command contract. No failed mutation triggers a fallback.

use super::inspect::{GITHUB_COPILOT_PLUGIN_ID, MARKETPLACE_NAME};

pub(crate) enum Command<'a> {
    List,
    MarketplaceList,
    Enable,
    Disable,
    Install,
    Remove,
    MarketplaceAdd(&'a str),
    MarketplaceRemove,
}

impl<'a> Command<'a> {
    pub(crate) fn arguments(self) -> Vec<&'a str> {
        let family = if cfg!(windows) { "plugin" } else { "plugins" };
        match self {
            Self::List if cfg!(windows) => vec![family, "list", "--json"],
            Self::List => vec![family, "list", "--kind", "plugin", "--json"],
            Self::MarketplaceList => vec![family, "marketplace", "list", "--json"],
            Self::MarketplaceAdd(root) => vec![family, "marketplace", "add", root],
            Self::MarketplaceRemove => vec![family, "marketplace", "remove", MARKETPLACE_NAME],
            Self::Install => vec![family, "install", GITHUB_COPILOT_PLUGIN_ID],
            action => {
                let verb = match action {
                    Self::Enable => "enable",
                    Self::Disable => "disable",
                    Self::Remove if cfg!(windows) => "uninstall",
                    Self::Remove => "remove",
                    _ => unreachable!("read and marketplace commands are handled above"),
                };
                let mut arguments = vec![family, verb, GITHUB_COPILOT_PLUGIN_ID];
                if !cfg!(windows) {
                    arguments.push("--plugin");
                }
                arguments
            }
        }
    }

    pub(crate) fn owned_arguments(self) -> Vec<String> {
        self.arguments().into_iter().map(str::to_owned).collect()
    }
}
