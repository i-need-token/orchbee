//! One retained Client batch joins a bounded canonical Agent page.
use super::{
    AgentViewFacts, ArmedSignals, Duration, FailureKind, Finish, Instant, Locale, Output,
    OutputFormat, QueryCommand, SignalWatch, check, classify, current_client, failure_output,
    query, read_many, results, success_output, view_failure,
};
use orchbee_agent_observation_protocol::AgentListOutcome;

pub(in crate::cli::agent_view) fn run(
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let QueryCommand::AgentList {
        client: Some(selected),
        include_ended,
        limit,
        cursor,
    } = command
    else {
        return failure_output(
            view_failure(FailureKind::InternalFailure),
            Some(format),
            locale,
        );
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                view_failure(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let result = (|| {
        let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
        check(deadline, &mut signals)?;
        let (mut request_id, first) = query::read_agent_list(
            &mut client,
            *include_ended,
            *limit,
            cursor.clone(),
            deadline,
            signals.poll_fd(),
        )?;
        let AgentListOutcome::Listed {
            mut revision,
            mut next_cursor,
            mut items,
        } = first
        else {
            return Err(view_failure(FailureKind::ProtocolInvalid));
        };
        let mut joined = Vec::with_capacity(items.len());
        if items.is_empty() {
            current_client(&mut client, &snapshot, selected, deadline, &mut signals)?;
        } else {
            let ids = items
                .iter()
                .map(|item| item.session_id.clone())
                .collect::<Vec<_>>();
            let attention = read_many(
                &mut client,
                &snapshot,
                selected,
                &ids,
                deadline,
                &mut signals,
            )?;
            let (final_id, final_page) = query::read_agent_list(
                &mut client,
                *include_ended,
                *limit,
                cursor.clone(),
                deadline,
                signals.poll_fd(),
            )?;
            let AgentListOutcome::Listed {
                revision: final_revision,
                next_cursor: final_cursor,
                items: final_items,
            } = final_page
            else {
                return Err(view_failure(FailureKind::ProtocolInvalid));
            };
            if final_revision.value() < revision.value() {
                return Err(view_failure(FailureKind::ProtocolInvalid));
            }
            request_id = final_id;
            revision = final_revision;
            next_cursor = final_cursor;
            items = final_items;
            if !items.is_empty() {
                let final_ids = items
                    .iter()
                    .map(|item| item.session_id.clone())
                    .collect::<Vec<_>>();
                let changed = query::attention_changed_sessions(
                    &mut client,
                    &attention.attention,
                    revision,
                    &final_ids,
                    deadline,
                    signals.poll_fd(),
                )
                .map_err(|failure| failure.failure_schema(crate::cli::schema::AGENT_FAILURE_ID))?;
                for item in items {
                    let unchanged = changed
                        .as_ref()
                        .is_some_and(|ids| !ids.contains(&item.session_id));
                    let view = attention.project(
                        &snapshot,
                        revision,
                        AgentViewFacts::summary(&item),
                        unchanged,
                    )?;
                    joined.push((item, view));
                }
            }
        }
        check(deadline, &mut signals)?;
        Ok((
            results::agent_source(&request_id),
            results::Projection::AgentClientList(Box::new(results::AgentClientListDoc {
                client_id: selected.clone(),
                revision,
                next_cursor,
                items: joined,
            })),
        ))
    })();
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(view_failure(FailureKind::Cancelled))
        }
        Err(_) => Err(view_failure(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
        Err(failure) => failure_output(classify(failure), Some(format), locale),
    };
    output.finalization = finalization;
    output
}
