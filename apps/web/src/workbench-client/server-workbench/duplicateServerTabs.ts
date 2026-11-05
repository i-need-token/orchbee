import type { ActionResult } from '@orchbee/actions';
import type {
	WorkbenchAuthorityContext,
	WorkbenchAuthorityTabDuplication,
	WorkbenchAuthorityTabDuplicationReport,
	WorkbenchTabDuplicationStop,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from './ServerWorkbenchSession';
import { DEFAULT_TERMINAL_GEOMETRY } from './workspace-launch';

type DuplicationSession = Pick<ServerWorkbenchSession, 'ask' | 'published' | 'snapshot'>;
type TabDuplicateResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.tab-duplicate.result' }
>;
type TabDuplicateOutcome = TabDuplicateResult['outcome'];
type TabDuplicateReport = Extract<TabDuplicateOutcome, { readonly kind: 'available' }>;
type TabDuplicateFailure = Extract<
	TabDuplicateReport['status'],
	{ readonly kind: 'failed' }
>['reason'];
type TabDuplicateAction = Extract<
	Parameters<DuplicationSession['ask']>[0],
	{ readonly kind: 'workbench.tab-duplicate' }
>['action'];

/** How long this Client follows a duplication: the Server's 120 seconds and room for a last read. */
const OBSERVATION_BUDGET_MILLISECONDS = 130_000;
/** As in the TUI, a pending duplication, or a Start whose reply was lost, is asked again this often. */
const OBSERVATION_INTERVAL_MILLISECONDS = 100;
/** A newer revision that left every source as shown is not a refusal of the duplication. */
const STALE_RETRY_LIMIT = 4;

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/** The sources in the order the snapshot shows them, or null once any of them is gone. */
function heldSources(snapshot: WorkbenchSnapshot, duplication: WorkbenchAuthorityTabDuplication) {
	const context = snapshot.terminalContexts.find(
		(candidate) => candidate.terminalContextId === duplication.terminalContextId,
	);
	if (context === undefined) return null;
	const requested = new Set(duplication.tabIds);
	const tabIds = context.tabs.filter((tab) => requested.has(tab.tabId)).map((tab) => tab.tabId);
	return tabIds.length === duplication.tabIds.length
		? { tabIds, terminalContextId: context.terminalContextId }
		: null;
}

function askTabDuplicate(
	session: DuplicationSession,
	action: TabDuplicateAction,
	timeoutMilliseconds: number,
): Promise<ServerWorkbenchAnswer> {
	return session.ask(
		{
			action,
			kind: 'workbench.tab-duplicate',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
		},
		timeoutMilliseconds,
	);
}

function duplicateOutcome(answer: ServerWorkbenchAnswer): TabDuplicateOutcome | null {
	return answer.kind === 'answered' && answer.response.kind === 'workbench.tab-duplicate.result'
		? answer.response.outcome
		: null;
}

/** How the TUI words each way the Server stops a duplication. */
function stopOf(reason: TabDuplicateFailure): WorkbenchTabDuplicationStop {
	switch (reason) {
		case 'source-unavailable':
			return 'changed';
		case 'copy-refused':
			return 'not-created';
		case 'deadline':
			return 'deadline';
		case 'terminal-not-started':
		case 'outcome-unknown':
		case 'session-stopping':
		case 'internal':
			return 'terminal-not-started';
	}
}

/** What a Start the Server did not take says; it retained nothing, so nothing was copied. */
function startRefused(
	outcome: Exclude<TabDuplicateOutcome, TabDuplicateReport>,
): WorkbenchTabDuplicationStop {
	switch (outcome.kind) {
		case 'rejected':
			return outcome.reason === 'resource-limit' ? 'not-created' : 'changed';
		case 'denied':
		case 'unavailable':
			return 'not-created';
		case 'conflict':
		case 'not-found':
			return 'terminal-not-started';
	}
}

function copiesOf(report: TabDuplicateReport): number {
	return report.members.filter((member) => member.copy !== null).length;
}

function lastCopyRevision(report: TabDuplicateReport): string | null {
	let last: bigint | null = null;
	for (const { copy } of report.members) {
		if (copy === null) continue;
		const revision = BigInt(copy.revision);
		if (last === null || revision > last) last = revision;
	}
	return last === null ? null : last.toString();
}

/**
 * Several Tabs duplicate as one Server operation, which copies them one at a time, each right after
 * itself, and stops at the first it cannot copy with a started terminal; this Client starts it once
 * and reads it. A Start whose reply was lost goes again unchanged and observes the same operation,
 * and one a newer revision turned away goes again while every source is still shown in order. As
 * the TUI does, the person stays where they are, and nothing they do afterwards stops it.
 */
export async function duplicateServerTabs(
	session: DuplicationSession,
	context: WorkbenchAuthorityContext,
	duplication: WorkbenchAuthorityTabDuplication,
): Promise<ActionResult<WorkbenchAuthorityTabDuplicationReport>> {
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	const total = duplication.tabIds.length;
	const stopped = (
		stop: WorkbenchTabDuplicationStop,
		created: number,
	): ActionResult<WorkbenchAuthorityTabDuplicationReport> => ({
		kind: 'completed',
		value: { created, kind: 'stopped', stop, total },
	});
	const operationId = createWorkbenchRequestId();
	let start: TabDuplicateAction | null = null;
	let delivered = false;
	let resends = 0;
	let report: TabDuplicateReport | null = null;
	while (report === null) {
		if (start === null) {
			const { snapshot } = session;
			const held = heldSources(snapshot, duplication);
			if (held === null) return stopped('changed', 0);
			if (context.signal.aborted) return { kind: 'cancelled' };
			start = {
				expectedRevision: snapshot.revision,
				geometry: DEFAULT_TERMINAL_GEOMETRY,
				kind: 'start',
				operationId,
				...held,
			};
		}
		const remaining = deadline - performance.now();
		if (remaining <= 0) return stopped(delivered ? 'terminal-not-started' : 'deadline', 0);
		context.markCommitted();
		const answer = await askTabDuplicate(session, start, remaining);
		if (answer.kind === 'not-submitted') {
			return stopped(delivered ? 'terminal-not-started' : 'not-created', 0);
		}
		const outcome = duplicateOutcome(answer);
		if (outcome === null) {
			// The Start may have been taken; the same Start observes it instead of copying again.
			delivered = true;
			await delay(OBSERVATION_INTERVAL_MILLISECONDS);
			continue;
		}
		if (outcome.kind === 'available') {
			report = outcome;
		} else if (
			outcome.kind === 'rejected' &&
			outcome.reason === 'revision-mismatch' &&
			resends < STALE_RETRY_LIMIT
		) {
			// A refusal retains nothing, so the same operation identity may start again.
			await session.published(outcome.revision);
			resends += 1;
			start = null;
		} else {
			return stopped(startRefused(outcome), 0);
		}
	}
	for (;;) {
		const { status } = report;
		if (status.kind !== 'pending') {
			// The copies arrive with their publications before the outcome is told.
			const revision = lastCopyRevision(report);
			if (revision !== null) await session.published(revision);
			if (status.kind === 'failed') return stopped(stopOf(status.reason), copiesOf(report));
			return { kind: 'completed', value: { kind: 'duplicated' } };
		}
		const remaining = deadline - performance.now();
		if (remaining <= 0) return stopped('deadline', copiesOf(report));
		await delay(Math.min(OBSERVATION_INTERVAL_MILLISECONDS, remaining));
		const answer = await askTabDuplicate(session, { kind: 'read', operationId }, remaining);
		if (answer.kind === 'not-submitted') {
			return stopped('terminal-not-started', copiesOf(report));
		}
		const next = duplicateOutcome(answer);
		if (next === null) continue;
		if (next.kind !== 'available') return stopped('terminal-not-started', copiesOf(report));
		report = next;
	}
}
