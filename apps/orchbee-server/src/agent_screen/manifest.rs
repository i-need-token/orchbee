//! Strict bundled manifest admission and compiled priority/gate evaluation; no filesystem overrides.
use super::{Detection, ScreenInput, regions::Region};
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::PanelAgentState;
use regex::{Regex, RegexBuilder};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    id: String,
    #[serde(rename = "version")]
    _version: Option<String>,
    min_engine_version: Option<u32>,
    #[serde(rename = "updated_at")]
    _updated_at: Option<String>,
    #[serde(default, rename = "aliases")]
    _aliases: Vec<String>,
    #[serde(default)]
    rules: Vec<RawRule>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    state: Option<PanelAgentState>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_region")]
    region: String,
    #[serde(default)]
    visible_idle: bool,
    #[serde(default)]
    visible_blocker: bool,
    #[serde(default)]
    visible_working: bool,
    #[serde(default)]
    skip_state_update: bool,
    #[serde(default)]
    all: Vec<RawGate>,
    #[serde(default)]
    any: Vec<RawGate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<RawGate>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGate {
    #[serde(default)]
    all: Vec<Self>,
    #[serde(default)]
    any: Vec<Self>,
    #[serde(default, rename = "not")]
    not_gate: Vec<Self>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}
fn default_region() -> String {
    "whole_recent".into()
}
struct Gate {
    all: Vec<Self>,
    any: Vec<Self>,
    not: Vec<Self>,
    contains: Vec<String>,
    regex: Vec<Regex>,
    lines: Vec<Regex>,
}
struct Rule {
    priority: i32,
    region: Region,
    gate: Gate,
    result: Detection,
}
pub(super) struct Manifest {
    pub(super) provider: AgentReportProvider,
    rules: Vec<Rule>,
}
#[derive(Default)]
struct Complexity {
    gates: usize,
    matchers: usize,
}
impl Gate {
    fn compile(
        raw: RawGate,
        depth: usize,
        negative: bool,
        count: &mut Complexity,
    ) -> Result<Self, String> {
        count.gates += 1;
        let size = raw.contains.len() + raw.regex.len() + raw.line_regex.len();
        count.matchers += size;
        if depth > 8 || count.gates > 512 || size > 32 || count.matchers > 1024 {
            return Err("Matcher complexity exceeds bound".into());
        }
        let positive = size > 0 || !raw.all.is_empty() || !raw.any.is_empty();
        if !positive && (!negative || raw.not_gate.is_empty()) {
            return Err("Empty or exclusively negative positive gate".into());
        }
        if raw
            .contains
            .iter()
            .chain(&raw.regex)
            .chain(&raw.line_regex)
            .any(|s| s.chars().count() > 512)
        {
            return Err("Matcher exceeds character bound".into());
        }
        let compile = |p: &String| {
            RegexBuilder::new(p)
                .size_limit(1024 * 1024)
                .dfa_size_limit(1024 * 1024)
                .build()
                .map_err(|e| e.to_string())
        };
        Ok(Self {
            contains: raw.contains.into_iter().map(|s| s.to_lowercase()).collect(),
            regex: raw.regex.iter().map(compile).collect::<Result<_, _>>()?,
            lines: raw
                .line_regex
                .iter()
                .map(compile)
                .collect::<Result<_, _>>()?,
            all: raw
                .all
                .into_iter()
                .map(|g| Self::compile(g, depth + 1, false, count))
                .collect::<Result<_, _>>()?,
            any: raw
                .any
                .into_iter()
                .map(|g| Self::compile(g, depth + 1, false, count))
                .collect::<Result<_, _>>()?,
            not: raw
                .not_gate
                .into_iter()
                .map(|g| Self::compile(g, depth + 1, true, count))
                .collect::<Result<_, _>>()?,
        })
    }
    fn matches(&self, text: &str, lower: &str) -> bool {
        self.contains.iter().all(|s| lower.contains(s))
            && self.regex.iter().all(|r| r.is_match(text))
            && self
                .lines
                .iter()
                .all(|r| text.lines().any(|line| r.is_match(line)))
            && self.all.iter().all(|g| g.matches(text, lower))
            && (self.any.is_empty() || self.any.iter().any(|g| g.matches(text, lower)))
            && !self.not.iter().any(|g| g.matches(text, lower))
    }
}
impl Manifest {
    pub(super) fn parse(provider: AgentReportProvider, text: &str) -> Result<Self, String> {
        if text.len() > 256 * 1024 {
            return Err("Manifest exceeds byte bound".into());
        }
        let raw: RawManifest = toml::from_str(text).map_err(|e| e.to_string())?;
        if AgentReportProvider::lookup(&raw.id) != Some(provider)
            || raw.min_engine_version.is_some_and(|v| v > 3)
            || raw.rules.is_empty()
            || raw.rules.len() > 128
        {
            return Err("Invalid manifest identity, version or rule count".into());
        }
        let mut count = Complexity::default();
        let mut rules = Vec::new();
        let version = raw.min_engine_version;
        for raw in raw.rules {
            if raw.id.trim().is_empty()
                || (raw.skip_state_update
                    && (raw.state != Some(PanelAgentState::Unknown)
                        || raw.visible_idle
                        || raw.visible_blocker
                        || raw.visible_working))
            {
                return Err("Invalid rule identity or skip state".into());
            }
            let region = Region::parse(&raw.region)?;
            if matches!(region, Region::TopNonEmpty(_)) && version.is_some_and(|v| v < 3) {
                return Err("Top region requires engine version 3".into());
            }
            let state = raw.state.unwrap_or(PanelAgentState::Unknown);
            let result = Detection {
                state,
                skip: raw.skip_state_update,
                visible_idle: raw.visible_idle && state == PanelAgentState::Idle,
                visible_blocker: raw.visible_blocker && state == PanelAgentState::Blocked,
                visible_working: raw.visible_working && state == PanelAgentState::Working,
            };
            let gate = Gate::compile(
                RawGate {
                    all: raw.all,
                    any: raw.any,
                    not_gate: raw.not_gate,
                    contains: raw.contains,
                    regex: raw.regex,
                    line_regex: raw.line_regex,
                },
                0,
                false,
                &mut count,
            )?;
            rules.push(Rule {
                priority: raw.priority,
                region,
                gate,
                result,
            });
        }
        Ok(Self { provider, rules })
    }
    pub(super) fn detect(&self, input: ScreenInput<'_>) -> Detection {
        let mut winner: Option<&Rule> = None;
        for rule in &self.rules {
            let text = rule.region.select(input);
            if winner.is_none_or(|w| rule.priority > w.priority)
                && rule.gate.matches(text, &text.to_lowercase())
            {
                winner = Some(rule);
            }
        }
        winner.map_or_else(|| Detection::fallback(true), |r| r.result)
    }
}
pub(super) fn bundled() -> Result<Vec<Manifest>, String> {
    macro_rules! bundle { ($($provider:ident => $file:literal),* $(,)?) => {vec![$((AgentReportProvider::$provider,include_str!(concat!("../../resources/agent-detection/",$file,".toml")))),*]}; }
    bundle!(Amp=>"amp",Antigravity=>"antigravity",Claude=>"claude",Cline=>"cline",Codex=>"codex",Cursor=>"cursor",Devin=>"devin",Droid=>"droid",Gemini=>"gemini",Grok=>"grok",Hermes=>"hermes",Kilo=>"kilo",Kimi=>"kimi",Kiro=>"kiro",Letta=>"letta",Maki=>"maki",Muse=>"muse",OpenCode=>"opencode",Pi=>"pi",Qoder=>"qodercli",Qwen=>"qwen",Copilot=>"github-copilot")
        .into_iter().map(|(p,text)|Manifest::parse(p,text)).collect()
}
