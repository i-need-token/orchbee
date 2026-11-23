//! The page's categories and groups in the Desktop page's order, and the entries a category, a
//! search, or a child page lists. Values only decide which dependent rows show.
use super::{Choice, Page, Row};
use crate::localization::{Locale, Words, words};
use orchbee_workbench_protocol::{
    AgentStartSelector, CompletionReminderMode, TerminalDisplayAppearance,
};

/// One entry of the page's left column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    General,
    Appearance,
    Themes,
    Terminal,
    Notifications,
    Keyboard,
    Agents,
    /// The computers that run Workspaces: this one and the saved machines; it holds no setting.
    Machines,
    /// This Client's version, the update control, and Automatic updates.
    About,
}

/// Every category, in the left column's order.
const EVERY: [Category; 9] = [
    Category::General,
    Category::Appearance,
    Category::Themes,
    Category::Terminal,
    Category::Notifications,
    Category::Keyboard,
    Category::Agents,
    Category::Machines,
    Category::About,
];

/// The categories where this platform saves no machines.
const WITHOUT_MACHINES: [Category; 8] = [
    Category::General,
    Category::Appearance,
    Category::Themes,
    Category::Terminal,
    Category::Notifications,
    Category::Keyboard,
    Category::Agents,
    Category::About,
];

impl Category {
    /// The categories this platform shows, in order: Machines only where machines can be saved.
    pub(crate) fn all() -> &'static [Self] {
        if crate::host_capabilities::Capability::SavedMachines.available() {
            &EVERY
        } else {
            &WITHOUT_MACHINES
        }
    }

    pub(crate) const fn groups(self) -> &'static [Group] {
        match self {
            Self::General => &[Group::Language],
            Self::Appearance => &[Group::AppAppearance, Group::TerminalColors],
            Self::Themes => &[Group::LightThemes, Group::DarkThemes],
            Self::Terminal => &[
                Group::SelectionKeys,
                Group::Output,
                Group::AfterRestart,
                Group::LaunchProfiles,
            ],
            Self::Notifications => &[Group::WhenToNotify, Group::SoundDelivery],
            Self::Keyboard => &[Group::Prefix],
            // Launch options list every Agent, so they come last, as in the window's Settings.
            Self::Agents => &[Group::AgentConfirmations, Group::AgentLaunch],
            Self::Machines => &[Group::MachineList],
            Self::About => &[Group::About],
        }
    }

    pub(crate) const fn name(self, locale: Locale) -> &'static str {
        locale.words(match self {
            Self::General => words("General", "通用"),
            Self::Appearance => words("Appearance", "外观"),
            Self::Themes => words("Themes", "主题"),
            Self::Terminal => words("Terminal", "终端"),
            Self::Notifications => words("Notifications", "通知"),
            Self::Keyboard => words("Keyboard", "快捷键"),
            Self::Agents => words("Agents", "智能体"),
            Self::Machines => words("Machines", "机器"),
            Self::About => words("About", "关于"),
        })
    }

    /// Every category of settings but Themes ends with the line that points to the settings
    /// file; Machines holds no setting and About's one setting needs no pointer.
    pub(crate) const fn config_file(self) -> bool {
        !matches!(self, Self::Themes | Self::Machines | Self::About)
    }

    /// Search finds settings, so it skips Machines.
    const fn searched(self) -> bool {
        !matches!(self, Self::Machines)
    }
}

/// One framed group of rows under its heading; a few groups have no heading of their own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    Language,
    AfterRestart,
    AppAppearance,
    TerminalColors,
    LightThemes,
    DarkThemes,
    SelectionKeys,
    Output,
    LaunchProfiles,
    WhenToNotify,
    SoundDelivery,
    Prefix,
    AgentLaunch,
    /// One row for each Agent whose routine confirmations Orchbee can answer.
    AgentConfirmations,
    /// About's version, update and Automatic updates rows; unnamed, since the category names it.
    About,
    /// Settings › Machines: this computer and one row per saved machine; unnamed, since the
    /// category names it.
    MachineList,
    /// A machine's page: what it is and how it is connected.
    Machine,
    /// A machine's page: what ends its terminals or its record.
    MachineEnd,
}

/// A group's own lines: its heading, the line under the heading, and the line it ends with.
#[derive(Clone, Copy)]
struct GroupWords {
    heading: Option<Words>,
    description: Option<Words>,
    note: Option<Words>,
}

/// A group framed without a heading.
const UNNAMED: GroupWords = GroupWords {
    heading: None,
    description: None,
    note: None,
};

const fn titled(en_us: &'static str, zh_cn: &'static str) -> GroupWords {
    GroupWords {
        heading: Some(words(en_us, zh_cn)),
        ..UNNAMED
    }
}

impl GroupWords {
    const fn described(mut self, en_us: &'static str, zh_cn: &'static str) -> Self {
        self.description = Some(words(en_us, zh_cn));
        self
    }

    const fn noted(mut self, en_us: &'static str, zh_cn: &'static str) -> Self {
        self.note = Some(words(en_us, zh_cn));
        self
    }
}

impl Group {
    /// The one declaration of a group's words. A new group is a variant above, its arm here, its
    /// rows below, and its place in a category's list.
    const fn words(self) -> GroupWords {
        match self {
            Self::Language | Self::Prefix | Self::About | Self::Machine | Self::MachineEnd => {
                UNNAMED
            }
            Self::MachineList => UNNAMED.noted("No machine matches.", "没有匹配的机器。"),
            Self::AfterRestart => titled("After a restart", "重启之后"),
            Self::AppAppearance => titled("App appearance", "应用外观"),
            Self::TerminalColors => titled("Terminal colors", "终端颜色"),
            Self::LightThemes => titled("Light theme", "浅色主题").described(
                "Applies when the app appearance is Light.",
                "应用外观为浅色时才会使用。",
            ),
            Self::DarkThemes => titled("Dark theme", "深色主题").described(
                "Applies when the app appearance is Dark.",
                "应用外观为深色时才会使用。",
            ),
            Self::SelectionKeys => titled("Selection and keys", "选择与按键"),
            Self::Output => titled("Output", "输出"),
            Self::LaunchProfiles => titled("Launch profiles", "启动配置").described(
                "Which shell a new terminal runs, plus your own profiles.",
                "新终端运行哪个 Shell，以及你自己的启动配置。",
            ),
            Self::WhenToNotify => titled("When to notify", "何时通知"),
            Self::SoundDelivery => titled("Sound and delivery", "声音与显示"),
            Self::AgentLaunch => titled("Launch options", "启动选项").described(
                "Applies to new conversations started by Orchbee.",
                "应用于之后通过 Orchbee 新建的会话。",
            ),
            Self::AgentConfirmations => titled("Allow routine confirmations", "自动允许常规确认")
                .described(
                    "Presses the Agent's allow-once for you.",
                    "替你按下智能体的单次允许。",
                )
                .noted(
                    "Request more Agents in a GitHub issue.",
                    "可在 GitHub issue 中申请更多智能体。",
                ),
        }
    }

    /// The group's heading; `None` for a group framed without one.
    pub(crate) fn heading(self, locale: Locale) -> Option<&'static str> {
        self.words().heading.map(|heading| locale.words(heading))
    }

    /// The line under a described group's heading.
    pub(crate) fn description(self, locale: Locale) -> Option<&'static str> {
        self.words()
            .description
            .map(|description| locale.words(description))
    }

    /// The line a noted group ends with, under its rows.
    pub(crate) fn note(self, locale: Locale) -> Option<&'static str> {
        self.words().note.map(|note| locale.words(note))
    }

    /// The rows this group can show, in order; theme lists come from the shipped catalog.
    pub(crate) fn rows(self) -> Vec<Row> {
        match self {
            Self::Language => vec![Row::Language],
            Self::AfterRestart => vec![Row::RestoreTerminals],
            Self::AppAppearance => vec![Row::Theme],
            Self::TerminalColors => vec![Row::TerminalTheme, Row::Foreground, Row::Background],
            Self::LightThemes => theme_rows(Row::LightTheme),
            Self::DarkThemes => theme_rows(Row::DarkTheme),
            Self::SelectionKeys => vec![Row::CopyOnSelect],
            Self::Output => vec![Row::Scrollback],
            Self::LaunchProfiles => vec![Row::LaunchProfiles],
            Self::WhenToNotify => vec![
                Row::Reminders,
                Row::CompletionReminders,
                Row::CompletionThreshold,
            ],
            Self::SoundDelivery => vec![Row::Sound],
            Self::Prefix => vec![Row::PrefixKey],
            Self::AgentLaunch => {
                let mut providers: Vec<_> = AgentStartSelector::all().collect();
                providers.sort_by_cached_key(|provider| {
                    (
                        crate::agent_provider::popularity_rank(provider.agent().id()),
                        Row::AgentLaunchProvider(*provider)
                            .name(Locale::EnUs)
                            .to_ascii_lowercase(),
                    )
                });
                providers
                    .into_iter()
                    .map(Row::AgentLaunchProvider)
                    .collect()
            }
            Self::AgentConfirmations => vec![
                Row::AutoAllowClaude,
                Row::AutoAllowCodex,
                Row::AutoAllowGemini,
                Row::AutoAllowOpenCode,
            ],
            Self::About => vec![Row::Version, Row::Updates, Row::AutomaticUpdates],
            // The saved machines' own rows come from the facts the TUI reports.
            Self::MachineList => vec![Row::AddMachine, Row::FindMachine, Row::ThisComputer],
            Self::Machine => vec![
                Row::MachineName,
                Row::MachineAddress,
                Row::MachineStatus,
                Row::MachineFree,
                Row::MachinePassword,
            ],
            Self::MachineEnd => vec![Row::MachineStop, Row::MachineRemove],
        }
    }

    pub(crate) const fn is_theme_list(self) -> bool {
        matches!(self, Self::LightThemes | Self::DarkThemes)
    }
}

fn theme_rows(slot: Row) -> Vec<Row> {
    Choice::options(slot)
        .iter()
        .filter_map(|choice| match choice {
            Choice::ColorTheme(theme) => Some(Row::ThemeOption(*theme)),
            _ => None,
        })
        .collect()
}

/// One line group of the right column. Headings and frame edges never take focus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    /// A group's heading: the top edge of its frame, with its title and description.
    Heading(Group),
    Setting(Row),
    /// One option of the open choice list, under its row.
    Option(Choice),
    /// The line a group ends with, under its rows.
    Note(Group),
    /// The bottom edge of a group's frame.
    End,
    /// The closing line that points to the settings file.
    ConfigFile,
    /// The search found nothing.
    NoResults,
}

/// What decides which entries show: dependent rows hide until their setting applies.
#[derive(Clone, Copy)]
pub(super) struct Filter<'a> {
    pub(super) query: &'a str,
    pub(super) custom_colors: bool,
    pub(super) completion_reminders: bool,
    /// Rows whose key the settings registry does not yet mark for the page show anyway.
    pub(super) unlisted: bool,
}

impl Filter<'_> {
    pub(super) fn of<'a>(
        query: &'a str,
        values: &orchbee_workbench_protocol::UserSettingsValues,
        unlisted: bool,
    ) -> Filter<'a> {
        Filter {
            query,
            custom_colors: values.terminal_display_theme == TerminalDisplayAppearance::Custom,
            completion_reminders: values.completion_reminder_mode != CompletionReminderMode::Never,
            unlisted,
        }
    }

    fn shows(self, row: Row) -> bool {
        match row {
            Row::Foreground | Row::Background => self.custom_colors,
            Row::CompletionThreshold => self.completion_reminders,
            Row::AutomaticUpdates => self.unlisted || on_page("update.automatic"),
            // A search lists settings; About's version and update control are information.
            Row::Version | Row::Updates => self.query.split_whitespace().next().is_none(),
            _ => true,
        }
    }
}

/// Whether the settings registry marks `path` for the settings page. A row that waits on its mark
/// stays hidden until the key's feature ships.
fn on_page(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    orchbee_local_runtime::find_setting(&segments).is_some_and(|setting| setting.page)
}

/// The entries of a page: a category's framed groups, every matching row across categories
/// while a query is set, or a child page's rows.
pub(super) fn entries(
    page: Page,
    category: Category,
    filter: Filter<'_>,
    picker: Option<(Row, &'static [Choice])>,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    match page {
        Page::Profiles => {
            entries.extend(page.rows().iter().copied().map(Entry::Setting));
            return entries;
        }
        Page::AgentLaunch(provider) => {
            entries.push(Entry::Heading(Group::AgentLaunch));
            if provider.yolo_flag().is_some() {
                entries.push(Entry::Setting(Row::AgentYolo(provider)));
            }
            entries.extend([
                Entry::Setting(Row::AgentArguments(provider)),
                Entry::Setting(Row::AgentCommand(provider)),
                Entry::Setting(Row::Back),
                Entry::End,
            ]);
            return entries;
        }
        // A machine's page lists what the machine facts hold, which `machines` builds.
        Page::Machine => return entries,
        Page::General => {}
    }
    let terms: Vec<String> = filter
        .query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let categories: &[Category] = if terms.is_empty() {
        std::slice::from_ref(&category)
    } else {
        Category::all()
    };
    for category in categories
        .iter()
        .filter(|category| terms.is_empty() || category.searched())
    {
        for group in category.groups() {
            let rows: Vec<Row> = group
                .rows()
                .into_iter()
                .filter(|row| filter.shows(*row))
                .filter(|row| terms.is_empty() || matches(*row, *category, *group, &terms))
                .collect();
            if rows.is_empty() {
                continue;
            }
            entries.push(Entry::Heading(*group));
            for row in rows {
                entries.push(Entry::Setting(row));
                if let Some((open, options)) = picker
                    && open == row
                {
                    entries.extend(options.iter().copied().map(Entry::Option));
                }
            }
            if group.words().note.is_some() {
                entries.push(Entry::Note(*group));
            }
            entries.push(Entry::End);
        }
    }
    if terms.is_empty() {
        if category.config_file() {
            entries.push(Entry::ConfigFile);
        }
    } else if entries.is_empty() {
        entries.push(Entry::NoResults);
    }
    entries
}

/// Every term appears in the row's name, description, group or category, in either locale; a
/// theme line also matches its theme's name.
fn matches(row: Row, category: Category, group: Group, terms: &[String]) -> bool {
    let document = [Locale::EnUs, Locale::ZhCn]
        .map(|locale| {
            let name = match row {
                Row::ThemeOption(theme) => locale.settings_choice(Choice::ColorTheme(theme)),
                _ => row.name(locale),
            };
            format!(
                "{name} {} {} {}",
                row.description(locale),
                group.heading(locale).unwrap_or_default(),
                category.name(locale),
            )
        })
        .join(" ")
        .to_lowercase();
    terms.iter().all(|term| document.contains(term.as_str()))
}
