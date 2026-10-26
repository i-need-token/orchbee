//! Present one retained layout read as layout, neighbor or edge output without another request.
use super::{Answer, output};
use crate::cli_text::CliText;
use crate::{
    cli::{Failure, FailureKind, Output, failure_output},
    invocation::{LayoutQuery, OutputFormat},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPanelDirection, ClientViewControlStatus, ClientViewOutcome,
};
use serde_json::json;

pub(super) fn emit(
    answer: &Answer,
    query: LayoutQuery,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut document = answer.document();
    document["schema"] = json!("orchbee.panel.layout");
    document["query"] = match query {
        LayoutQuery::Layout => json!({"kind":"layout"}),
        LayoutQuery::Neighbor(direction) => json!({"kind":"neighbor","direction":direction}),
        LayoutQuery::Edges => json!({"kind":"edges"}),
    };
    let layout = answer.reply.as_ref().and_then(|reply| {
        let ClientViewOutcome::Operation { operation } = &reply.outcome else {
            return None;
        };
        let ClientViewControlStatus::LayoutRead { layout, .. } = &operation.status else {
            return None;
        };
        Some(layout.as_ref())
    });
    document["projection"] = layout.map_or(serde_json::Value::Null, |layout| match query {
        LayoutQuery::Layout => json!({"kind":"layout"}),
        LayoutQuery::Neighbor(direction) => json!({"kind":"neighbor","direction":direction,
            "sourceAvailable":layout.source_available,"neighborPanelId":layout.neighbors.get(direction)}),
        LayoutQuery::Edges => json!({"kind":"edges","sourceAvailable":layout.source_available,
            "left":layout.at_edge(ClientPanelDirection::Left),"right":layout.at_edge(ClientPanelDirection::Right),
            "up":layout.at_edge(ClientPanelDirection::Up),"down":layout.at_edge(ClientPanelDirection::Down)}),
    });
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            let text = layout.map_or_else(
                || CliText::new(locale).cli_client_control(&answer.document()),
                |layout| CliText::new(locale).cli_panel_layout(layout, query),
            );
            output::encode_text(&text)
        }
    };
    match encoded {
        Ok(stdout) => Output {
            status: if answer.exit_code() == 0
                && !matches!(query, LayoutQuery::Layout)
                && layout.is_some_and(|layout| !layout.source_available)
            {
                3
            } else {
                answer.exit_code()
            },
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}
