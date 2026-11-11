//! Strict bundled manifest admission and compiled priority/gate evaluation; no filesystem overrides.
use super::{Detection, ScreenInput, regions::Region};
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::{
    AgentDetectionEvaluation, AgentDetectionEvidence, AgentDetectionManifest, AgentDetectionRule,
    AgentManifestSource, AgentScreenExplanation, PanelAgentState,
};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    id: String,
    version: Option<String>,
    min_engine_version: Option<u32>,
    #[serde(rename = "updated_at")]
    _updated_at: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
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
    metadata: AgentDetectionRule,
    evidence: AgentDetectionEvidence,
    region: Region,
    gate: Gate,
    result: Detection,
}
pub(super) struct Manifest {
    pub(super) provider: AgentReportProvider,
    rules: Vec<Rule>,
    pub(super) version: Option<String>,
    pub(super) minimum_engine_version: Option<u32>,
    pub(super) provenance: AgentDetectionManifest,
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
        let matches_provider = |name: &str| {
            orchbee_agent_observation_protocol::normalize_agent_report_label(name).as_deref()
                == Some(provider.report_label())
        };
        if !(matches_provider(&raw.id) || raw.aliases.iter().any(|alias| matches_provider(alias)))
            || raw.min_engine_version.is_some_and(|v| v > 3)
            || raw.rules.is_empty()
            || raw.rules.len() > 128
            || raw
                .version
                .as_ref()
                .is_some_and(|v| super::version::Version::parse(v).is_none())
        {
            return Err("Invalid manifest identity, version or rule count".into());
        }
        let mut count = Complexity::default();
        let mut rules = Vec::new();
        let version = raw.min_engine_version;
        for raw in raw.rules {
            if raw.id.trim().is_empty()
                || raw.id.chars().count() > 128
                || raw.region.len() > 128
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
            if matches!(state, PanelAgentState::Done | PanelAgentState::Error) {
                return Err("Turn outcomes require explicit reports, not screen matchers".into());
            }
            let result = Detection {
                state,
                skip: raw.skip_state_update,
                visible_idle: raw.visible_idle && state == PanelAgentState::Idle,
                visible_blocker: raw.visible_blocker && state == PanelAgentState::Blocked,
                visible_working: raw.visible_working && state == PanelAgentState::Working,
            };
            let metadata = AgentDetectionRule {
                id: raw.id.clone(),
                priority: raw.priority,
                region: raw.region.clone(),
                state,
            };
            let gate_count =
                |n: usize| u16::try_from(n).map_err(|_| "Gate count exceeds bound".to_owned());
            let evidence = AgentDetectionEvidence {
                contains: raw.contains.clone(),
                regex: raw.regex.clone(),
                line_regex: raw.line_regex.clone(),
                all_count: gate_count(raw.all.len())?,
                any_count: gate_count(raw.any.len())?,
                not_count: gate_count(raw.not_gate.len())?,
                region_bytes: 0,
                region_preview: String::new(),
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
                metadata,
                evidence,
                region,
                gate,
                result,
            });
        }
        Ok(Self {
            provider,
            rules,
            version: raw.version.map(|v| v.trim().to_owned()),
            minimum_engine_version: raw.min_engine_version,
            provenance: AgentDetectionManifest {
                source: AgentManifestSource::Bundled,
                path: None,
                cached_remote_version: None,
                local_override_shadowing_remote: false,
                warnings: vec![],
            },
        })
    }
    pub(super) fn detect(&self, input: ScreenInput<'_>) -> Detection {
        self.evaluate(input, |_, _, _| {}).0
    }
    fn evaluate(
        &self,
        input: ScreenInput<'_>,
        mut visit: impl FnMut(&Rule, &str, bool),
    ) -> (Detection, Option<&Rule>) {
        let mut winner: Option<&Rule> = None;
        for rule in &self.rules {
            let text = rule.region.select(input);
            let matched = rule.gate.matches(text, &text.to_lowercase());
            visit(rule, text, matched);
            if matched && winner.is_none_or(|w| rule.metadata.priority > w.metadata.priority) {
                winner = Some(rule);
            }
        }
        (
            winner.map_or_else(|| Detection::fallback(true), |r| r.result),
            winner,
        )
    }
    pub(super) fn explain(&self, input: ScreenInput<'_>) -> AgentScreenExplanation {
        let mut evaluated_rules = Vec::with_capacity(self.rules.len());
        let (result, winner) = self.evaluate(input, |rule, text, matched| {
            let mut evidence = rule.evidence.clone();
            evidence.region_bytes = u32::try_from(text.len()).expect("screen input is bounded");
            evidence.region_preview = text.chars().take(240).collect();
            if text.chars().count() > 240 {
                evidence.region_preview.push_str("...");
            }
            evaluated_rules.push(AgentDetectionEvaluation {
                rule: rule.metadata.clone(),
                matched,
                evidence,
            });
        });
        let mut explanation = super::fallback_explanation(self.provider);
        explanation.manifest_source = Some(self.provenance.source.as_str().into());
        explanation.manifest = Some(self.provenance.clone());
        explanation.manifest_version = self.version.clone();
        explanation.state = result.state;
        explanation.matched_rule = winner.map(|r| r.metadata.clone());
        explanation.visible_idle = result.visible_idle;
        explanation.visible_blocker = result.visible_blocker;
        explanation.visible_working = result.visible_working;
        explanation.skip_state_update = result.skip;
        explanation.skipped_update_reason = winner
            .filter(|_| result.skip)
            .map(|r| format!("matched-rule:{}", r.metadata.id));
        if winner.is_some() {
            explanation.fallback_reason = None;
        }
        explanation.evaluated_rules = evaluated_rules;
        explanation
    }
}
pub(super) fn bundled() -> Result<Vec<Manifest>, String> {
    macro_rules! bundle { ($($provider:ident => $file:literal),* $(,)?) => {vec![$((AgentReportProvider::$provider,include_str!(concat!("../../resources/agent-detection/",$file,".toml")))),*]}; }
    bundle!(Amp=>"amp",Antigravity=>"antigravity",Claude=>"claude",Cline=>"cline",Codex=>"codex",Cursor=>"cursor",Devin=>"devin",Droid=>"droid",Gemini=>"gemini",Grok=>"grok",Hermes=>"hermes",Kilo=>"kilo",Kimi=>"kimi",Kiro=>"kiro",Letta=>"letta",Maki=>"maki",Muse=>"muse",Omp=>"omp",OpenCode=>"opencode",Pi=>"pi",Qoder=>"qodercli",Qwen=>"qwen",Copilot=>"github-copilot",AtomCode=>"atomcode",CodeBuddy=>"codebuddy",CommandCode=>"command-code",DevEco=>"deveco",Jcode=>"jcode",Mimo=>"mimo",MinimaxCode=>"minimax-code",Mothx=>"mothx",Penguin=>"penguin",PrimeAgent=>"prime-agent",Reasonix=>"reasonix",Snow=>"snow")
        .into_iter().map(|(p,text)|Manifest::parse(p,text)).collect()
}
