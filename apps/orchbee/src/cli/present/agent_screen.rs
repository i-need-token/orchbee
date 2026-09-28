//! Escaped diagnostic evidence keeps raw rule prediction separate from the effective state.
use super::{Bounded, Locale, TooLarge, escape, field};
use orchbee_workbench_protocol::TerminalAgentExplanation;

pub(super) fn render(
    out: &mut Bounded,
    terminal: &TerminalAgentExplanation,
    locale: Locale,
) -> Result<(), TooLarge> {
    let labels = locale.cli_agent_screen_fields();
    let explanation = &terminal.explanation;
    field(out, labels[0], &escape(&explanation.agent))?;
    field(
        out,
        locale.cli_reported_agent_fields()[0],
        &escape(terminal.view.agent.as_deref().unwrap_or("—")),
    )?;
    field(
        out,
        labels[1],
        locale.panel_report_state(terminal.view.state),
    )?;
    field(out, labels[2], locale.panel_report_state(explanation.state))?;
    field(
        out,
        labels[3],
        &escape(explanation.manifest_source.as_deref().unwrap_or("—")),
    )?;
    field(
        out,
        labels[4],
        &escape(explanation.manifest_version.as_deref().unwrap_or("—")),
    )?;
    field(
        out,
        labels[5],
        &escape(
            explanation
                .matched_rule
                .as_ref()
                .map_or("—", |r| r.id.as_str()),
        ),
    )?;
    if let Some(manifest) = &explanation.manifest {
        let fields = locale.cli_agent_manifest_fields();
        field(
            out,
            fields[0],
            &escape(manifest.path.as_deref().unwrap_or("—")),
        )?;
        field(
            out,
            fields[1],
            &escape(manifest.cached_remote_version.as_deref().unwrap_or("—")),
        )?;
        field(
            out,
            fields[2],
            locale.cli_agent_bool(manifest.local_override_shadowing_remote),
        )?;
        for warning in &manifest.warnings {
            field(out, fields[3], &locale.cli_agent_manifest_warning(warning))?;
        }
    }
    if explanation.screen_detection_skipped
        || explanation.skip_state_update
        || explanation.fallback_reason.is_some()
    {
        field(
            out,
            labels[6],
            locale.cli_agent_screen_reason(
                explanation.screen_detection_skipped,
                explanation.skip_state_update,
            ),
        )?;
    }
    for evaluated in &explanation.evaluated_rules {
        let rule = &evaluated.rule;
        out.write(format_args!("\n{}: {}\n", labels[7], escape(&rule.id)))?;
        field(out, labels[8], locale.cli_agent_bool(evaluated.matched))?;
        field(out, labels[9], &rule.priority.to_string())?;
        field(out, labels[10], &escape(&rule.region))?;
        field(out, labels[2], locale.panel_report_state(rule.state))?;
        field(
            out,
            labels[11],
            &evaluated.evidence.region_bytes.to_string(),
        )?;
        field(out, labels[12], &escape(&evaluated.evidence.region_preview))?;
        for (label, patterns) in [
            (labels[13], &evaluated.evidence.contains),
            (labels[14], &evaluated.evidence.regex),
            (labels[15], &evaluated.evidence.line_regex),
        ] {
            if !patterns.is_empty() {
                field(
                    out,
                    label,
                    &patterns
                        .iter()
                        .map(|p| escape(p))
                        .collect::<Vec<_>>()
                        .join(", "),
                )?;
            }
        }
        field(
            out,
            labels[16],
            &format!(
                "{}/{}/{}",
                evaluated.evidence.all_count,
                evaluated.evidence.any_count,
                evaluated.evidence.not_count
            ),
        )?;
    }
    Ok(())
}
