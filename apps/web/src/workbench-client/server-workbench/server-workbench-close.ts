import type { ActionResult } from '@orchbee/actions';
import type { WorkbenchAuthorityClosure, WorkbenchAuthorityContext } from '@orchbee/workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type {
	ServerWorkbenchAction,
	ServerWorkbenchAnswer,
	ServerWorkbenchOutcome,
	ServerWorkbenchSession,
} from './ServerWorkbenchSession';

type CloseSession = Pick<
	ServerWorkbenchSession,
	'ask' | 'published' | 'snapshot' | 'submit' | 'subscribe'
>;
type CloseOperationResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.close-operation.result' }
>;
type CloseStatus = Extract<
	CloseOperationResult['outcome'],
	{ readonly kind: 'available' }
>['status'];
type GroupCloseResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.workspace-group-close.result' }
>;
type GroupCloseAction = Extract<
	Parameters<CloseSession['ask']>[0],
	{ readonly kind: 'workbench.workspace-group-close' }
>['action'];
type CloseSubmission =
	| { readonly kind: 'answered'; readonly outcome: ServerWorkbenchOutcome }
	| { readonly kind: 'ended'; readonly result: ActionResult<void> };

/** The Action's context, which also knows whether any step has been handed over yet. */
interface CloseContext extends WorkbenchAuthorityContext {
	readonly handedOver: () => boolean;
}

/** How long this Client watches a close it started. The Server's work continues past it. */
const OBSERVATION_BUDGET_MILLISECONDS = 50_000;
/** Physical settlement has no progress notification, so only a pending close is sampled. */
const OBSERVATION_INTERVAL_MILLISECONDS = 50;
/** A newer revision that left the target untouched is not a refusal of the target. */
const STALE_RETRY_LIMIT = 4;

const NOT_SUBMITTED = { code: 'not-submitted', kind: 'blocked' } as const;
const SCOPE_CHANGED = { code: 'scope-changed', kind: 'blocked' } as const;

function failed(code: string, details?: string): ActionResult<void> {
	return { failure: details === undefined ? { code } : { code, details }, kind: 'failed' };
}

function workspaceFacts(snapshot: WorkbenchSnapshot, workspaceId: string): string {
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === workspaceId,
	);
	return JSON.stringify({
		context: snapshot.terminalContexts.find(
			(candidate) => candidate.terminalContextId === workspace?.terminalContextId,
		),
		workspace,
	});
}

function tabFacts(snapshot: WorkbenchSnapshot, terminalContextId: string, tabId: string): string {
	return (
		JSON.stringify(
			snapshot.terminalContexts
				.find((candidate) => candidate.terminalContextId === terminalContextId)
				?.tabs.find((candidate) => candidate.tabId === tabId),
		) ?? ''
	);
}

/** What a person confirming this close was told. Anything else changing does not concern them. */
function closureFacts(closure: WorkbenchAuthorityClosure, snapshot: WorkbenchSnapshot): string {
	switch (closure.type) {
		case 'workspace-close':
		case 'workspace-close-if-idle':
			return workspaceFacts(snapshot, closure.workspaceId);
		case 'project-workspaces-close':
			return JSON.stringify({
				members: snapshot.navigation.workspaces.filter(
					(workspace) => workspace.projectId === closure.projectId,
				),
				project: snapshot.navigation.projects.find(
					(project) => project.projectId === closure.projectId,
				),
			});
		case 'tabs-close':
			return JSON.stringify(
				closure.tabIds.map((tabId) => tabFacts(snapshot, closure.terminalContextId, tabId)),
			);
		case 'panel-close':
			return tabFacts(snapshot, closure.terminalContextId, closure.tabId);
	}
}

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function closeContext(context: WorkbenchAuthorityContext): CloseContext {
	let handedOver = false;
	return {
		handedOver: () => handedOver,
		markCommitted: () => {
			handedOver = true;
			context.markCommitted();
		},
		signal: context.signal,
	};
}

/** Hands one close over unless its target has changed since the facts were taken. */
async function submitClose(
	session: CloseSession,
	context: CloseContext,
	action: ServerWorkbenchAction | null,
	facts: (snapshot: WorkbenchSnapshot) => string,
	expected: string,
): Promise<CloseSubmission> {
	const submission = await session.submit((snapshot) =>
		facts(snapshot) === expected ? handOverServerWorkbenchAction(context, action) : null,
	);
	switch (submission.kind) {
		case 'settled':
			return { kind: 'answered', outcome: submission.outcome };
		case 'outcome-unknown':
			return { kind: 'ended', result: failed('outcome-unknown') };
		case 'not-submitted':
			if (facts(session.snapshot) !== expected) return { kind: 'ended', result: SCOPE_CHANGED };
			// Once a step has been handed over, cancelling can no longer say nothing happened.
			return {
				kind: 'ended',
				result:
					context.signal.aborted && !context.handedOver() ? { kind: 'cancelled' } : NOT_SUBMITTED,
			};
	}
}

/** A definite answer that ran nothing. A Server of this cohort answers a close in no other way. */
function refused(outcome: ServerWorkbenchOutcome): ActionResult<void> {
	switch (outcome.kind) {
		case 'rejected':
		case 'unavailable':
			return { code: outcome.reason, kind: 'blocked' };
		case 'denied':
			return { code: outcome.kind, kind: 'blocked' };
		default:
			return failed('outcome-unknown');
	}
}

/** What a close status means here; null while the close is still under way. */
function settledStatus(status: CloseStatus): ActionResult<void> | null {
	switch (status.kind) {
		case 'closing':
			return null;
		case 'closed':
			return { kind: 'completed' };
		case 'cleanup-blocked':
			return failed('cleanup-blocked');
		case 'outcome-unknown':
			return failed('outcome-unknown');
		case 'disposed-awaiting-publication':
			return !status.resumable &&
				(status.reason === 'preparing' || status.reason === 'target-cleanup-pending')
				? null
				: failed('awaiting-removal');
	}
}

/** Reads an accepted close by its original identity, which outlives the target it closed. */
async function observeClose(
	session: CloseSession,
	operationId: CloseOperationResult['operationId'],
	deadline: number,
): Promise<ActionResult<void>> {
	for (;;) {
		const remaining = deadline - performance.now();
		if (remaining <= 0) return failed('status-unavailable');
		const answer = await session.ask(
			{
				kind: 'workbench.close-operation.read',
				operationId,
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId: createWorkbenchRequestId(),
				sessionId: session.snapshot.sessionId,
			},
			remaining,
		);
		if (
			answer.kind !== 'answered' ||
			answer.response.kind !== 'workbench.close-operation.result' ||
			answer.response.outcome.kind !== 'available'
		) {
			return failed('status-unavailable');
		}
		const { status } = answer.response.outcome;
		if (status.kind === 'closed') await session.published(status.revision);
		const settled = settledStatus(status);
		if (settled !== null) return settled;
		await delay(Math.min(OBSERVATION_INTERVAL_MILLISECONDS, remaining));
	}
}

async function closeWorkspace(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<
		WorkbenchAuthorityClosure,
		{ readonly type: 'workspace-close' | 'workspace-close-if-idle' }
	>,
): Promise<ActionResult<void>> {
	const action = readServerWorkbenchAction(
		closure.type === 'workspace-close-if-idle'
			? { kind: 'workspace.close-if-idle', workspaceId: closure.workspaceId }
			: { kind: 'workspace.close', workspaceId: closure.workspaceId },
	);
	const factsOf = (snapshot: WorkbenchSnapshot): string => closureFacts(closure, snapshot);
	let expected = factsOf(session.snapshot);
	let repeatedOnChange = false;
	for (let attempt = 0; ; attempt += 1) {
		const submission = await submitClose(session, context, action, factsOf, expected);
		if (submission.kind === 'ended') return submission.result;
		const { outcome } = submission;
		if (outcome.kind === 'workspace-close-accepted') {
			return observeClose(
				session,
				outcome.closeRequestId,
				performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
			);
		}
		if (outcome.kind !== 'rejected') return refused(outcome);
		const current = factsOf(session.snapshot);
		if (outcome.reason === 'revision-mismatch') {
			if (current !== expected || attempt === STALE_RETRY_LIMIT) return SCOPE_CHANGED;
		} else if (outcome.reason !== 'confirmation-required' || current === expected) {
			return refused(outcome);
		} else if (repeatedOnChange) {
			// The reply described a Workspace that has moved on twice, so it answers nothing shown.
			return SCOPE_CHANGED;
		} else {
			repeatedOnChange = true;
			expected = current;
		}
	}
}

async function closeTabs(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'tabs-close' }>,
): Promise<ActionResult<void>> {
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	const { terminalContextId } = closure;
	const targets = closure.tabIds.map((tabId) => ({
		expected: tabFacts(session.snapshot, terminalContextId, tabId),
		tabId,
	}));
	for (const { expected, tabId } of targets) {
		const submission = await submitClose(
			session,
			context,
			readServerWorkbenchAction({ kind: 'tab.close', tabId, terminalContextId }),
			(snapshot) => tabFacts(snapshot, terminalContextId, tabId),
			expected,
		);
		if (submission.kind === 'ended') return submission.result;
		const { outcome } = submission;
		if (outcome.kind !== 'tab-close-accepted') return refused(outcome);
		const settled = await observeClose(session, outcome.closeRequestId, deadline);
		if (settled.kind !== 'completed') return settled;
	}
	return { kind: 'completed' };
}

async function closePanel(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'panel-close' }>,
): Promise<ActionResult<void>> {
	const factsOf = (snapshot: WorkbenchSnapshot): string => closureFacts(closure, snapshot);
	const submission = await submitClose(
		session,
		context,
		readServerWorkbenchAction({
			kind: 'panel.close',
			panelId: closure.panelId,
			tabId: closure.tabId,
			terminalContextId: closure.terminalContextId,
		}),
		factsOf,
		factsOf(session.snapshot),
	);
	if (submission.kind === 'ended') return submission.result;
	return submission.outcome.kind === 'panel-close-accepted'
		? { kind: 'completed' }
		: refused(submission.outcome);
}

function groupOutcome(answer: ServerWorkbenchAnswer): GroupCloseResult['outcome'] | null {
	return answer.kind === 'answered' &&
		answer.response.kind === 'workbench.workspace-group-close.result'
		? answer.response.outcome
		: null;
}

function askGroupClose(
	session: CloseSession,
	action: GroupCloseAction,
	requestId: GroupCloseResult['requestId'],
	timeoutMilliseconds?: number,
): Promise<ServerWorkbenchAnswer> {
	return session.ask(
		{
			action,
			kind: 'workbench.workspace-group-close',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: session.snapshot.sessionId,
		},
		timeoutMilliseconds,
	);
}

/** Where the last member's removal reached, so the read model holds the whole result. */
function lastClosedRevision(
	members: Extract<GroupCloseResult['outcome'], { readonly kind: 'available' }>['members'],
): string | null {
	let last: string | null = null;
	for (const member of members) {
		const status = member.close?.observation?.status;
		if (status?.kind === 'closed' && (last === null || BigInt(status.revision) > BigInt(last))) {
			last = status.revision;
		}
	}
	return last;
}

/** The Server owns the sequence and captures the members; this Client only watches it settle. */
async function closeProject(
	session: CloseSession,
	context: WorkbenchAuthorityContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'project-workspaces-close' }>,
): Promise<ActionResult<void>> {
	if (context.signal.aborted) return { kind: 'cancelled' };
	const project = session.snapshot.navigation.projects.find(
		(candidate) => candidate.projectId === closure.projectId,
	);
	if (project === undefined) return SCOPE_CHANGED;
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	const operationId = createWorkbenchRequestId();
	context.markCommitted();
	const started = await askGroupClose(
		session,
		{ kind: 'start', operationId, projectId: project.projectId },
		operationId,
	);
	if (started.kind === 'not-submitted') return NOT_SUBMITTED;
	let outcome = groupOutcome(started);
	if (outcome === null) return failed('outcome-unknown', operationId);
	if (outcome.kind !== 'available') {
		return {
			code: outcome.kind === 'unavailable' ? outcome.reason : outcome.kind,
			kind: 'blocked',
		};
	}
	for (;;) {
		const { status } = outcome;
		if (status.kind === 'closed') {
			const revision = lastClosedRevision(outcome.members);
			if (revision !== null) await session.published(revision);
			return { kind: 'completed' };
		}
		if (status.kind === 'failed') {
			return failed(
				status.reason === 'outcome-unknown' ? 'outcome-unknown' : 'close-stopped',
				operationId,
			);
		}
		const remaining = deadline - performance.now();
		if (remaining <= 0) return failed('status-unavailable', operationId);
		await delay(Math.min(OBSERVATION_INTERVAL_MILLISECONDS, remaining));
		const next = groupOutcome(
			await askGroupClose(
				session,
				{ kind: 'read', operationId },
				createWorkbenchRequestId(),
				remaining,
			),
		);
		if (next?.kind !== 'available') return failed('status-unavailable', operationId);
		outcome = next;
	}
}

/** Runs one close until its result is known, and never repeats one that may have run. */
export function closeServerWorkbench(
	session: CloseSession,
	context: WorkbenchAuthorityContext,
	closure: WorkbenchAuthorityClosure,
): Promise<ActionResult<void>> {
	switch (closure.type) {
		case 'workspace-close':
		case 'workspace-close-if-idle':
			return closeWorkspace(session, closeContext(context), closure);
		case 'project-workspaces-close':
			return closeProject(session, context, closure);
		case 'tabs-close':
			return closeTabs(session, closeContext(context), closure);
		case 'panel-close':
			return closePanel(session, closeContext(context), closure);
	}
}

export function watchServerWorkbenchClose(
	session: Pick<CloseSession, 'snapshot' | 'subscribe'>,
	closure: WorkbenchAuthorityClosure,
	onChanged: () => void,
): () => void {
	const expected = closureFacts(closure, session.snapshot);
	return session.subscribe((snapshot) => {
		if (closureFacts(closure, snapshot) !== expected) onChanged();
	});
}
