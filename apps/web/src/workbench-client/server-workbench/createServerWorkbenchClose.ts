import type { ActionResult } from '@orchbee/actions';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityContext,
	WorkbenchTabCloseStop,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerTabLeave } from './ServerLeavingTabs';
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
type CloseObservation = Extract<CloseOperationResult['outcome'], { readonly kind: 'available' }>;
type CloseStatus = CloseObservation['status'];
type ResumeOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.tab-close.resume.result' }
>['outcome'];
type GroupCloseResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.workspace-group-close.result' }
>;
type GroupCloseAction = Extract<
	Parameters<CloseSession['ask']>[0],
	{ readonly kind: 'workbench.workspace-group-close' }
>['action'];
type CloseSubmission =
	| {
			readonly kind: 'answered';
			readonly outcome: ServerWorkbenchOutcome;
			/** The snapshot the close was handed over against, which the Server decided at. */
			readonly decidedFrom: WorkbenchSnapshot;
	  }
	| { readonly kind: 'ended'; readonly result: ActionResult<void> };
type TabTarget = Pick<
	Extract<WorkbenchAuthorityClosure, { readonly type: 'tab-close-if-idle' }>,
	'tabId' | 'terminalContextId'
>;

/** The Action's context, which also knows whether any step has been handed over yet. */
interface CloseContext extends WorkbenchAuthorityContext {
	readonly handedOver: () => boolean;
}

/** A Tab close that stopped where an explicit Resume may continue it, with what Resume names. */
interface StoppedTabClose {
	readonly closeRequestId: CloseOperationResult['operationId'];
	readonly generation: CloseObservation['generation'];
	readonly stop: WorkbenchTabCloseStop;
	readonly target: Extract<CloseObservation['target'], { readonly kind: 'tab' }>;
}

/** How a followed close settled, and the stop a Resume may continue, if it stopped at one. */
interface ObservedClose {
	readonly result: ActionResult<void>;
	readonly stopped: StoppedTabClose | null;
}

/** A followed Tab close that ended without closing its Tab, which the Server may still hold open. */
interface UnsettledTabClose {
	readonly closeRequestId: CloseOperationResult['operationId'];
	/** Where an explicit Resume may continue it. */
	readonly stopped: StoppedTabClose | null;
}

/** What following a Tab's accepted close tells the window, and keeps for Resume and a later Close. */
interface TabCloseFollow {
	readonly leave: ServerTabLeave;
	/** Keeps how the Tab's followed close ended without closing it, or forgets that with null. */
	keep(terminalContextId: string, tabId: string, unsettled: UnsettledTabClose | null): void;
	/** The followed close of the Tab that last ended without closing it, while the Tab is there. */
	unsettled(terminalContextId: string, tabId: string): UnsettledTabClose | null;
}

/** How long this Client watches a close it started. The Server's work continues past it. */
const OBSERVATION_BUDGET_MILLISECONDS = 50_000;
/** Physical settlement has no progress notification, so only a pending close is sampled. */
const OBSERVATION_INTERVAL_MILLISECONDS = 50;
/** A newer revision that left the target untouched is not a refusal of the target. */
const STALE_RETRY_LIMIT = 4;
/** Unsettled Tab closes kept for Resume and a later Close; one person rarely leaves more behind. */
const MAXIMUM_UNSETTLED_TAB_CLOSES = 16;

const NOT_SUBMITTED = { code: 'not-submitted', kind: 'blocked' } as const;
const SCOPE_CHANGED = { code: 'scope-changed', kind: 'blocked' } as const;
const NO_LEAVE: ServerTabLeave = { leave: () => () => undefined };

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

function findTab(snapshot: WorkbenchSnapshot, terminalContextId: string, tabId: string) {
	return snapshot.terminalContexts
		.find((candidate) => candidate.terminalContextId === terminalContextId)
		?.tabs.find((candidate) => candidate.tabId === tabId);
}

function tabFacts(snapshot: WorkbenchSnapshot, terminalContextId: string, tabId: string): string {
	return JSON.stringify(findTab(snapshot, terminalContextId, tabId)) ?? '';
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
		case 'panel-close-if-idle':
		case 'tab-close-if-idle':
		case 'tab-close-resume':
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
	let decidedFrom = session.snapshot;
	const submission = await session.submit((snapshot) => {
		decidedFrom = snapshot;
		return facts(snapshot) === expected ? handOverServerWorkbenchAction(context, action) : null;
	});
	switch (submission.kind) {
		case 'settled':
			return { decidedFrom, kind: 'answered', outcome: submission.outcome };
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

/** Where only an explicit Resume continues a close, as the status says. */
function resumableStop(status: CloseStatus): WorkbenchTabCloseStop | null {
	switch (status.kind) {
		case 'cleanup-blocked':
			return status.resumable ? 'cleanup-blocked' : null;
		case 'disposed-awaiting-publication':
			return status.resumable ? 'awaiting-removal' : null;
		case 'closed':
		case 'closing':
		case 'outcome-unknown':
			return null;
	}
}

function stoppedTabClose(
	operationId: CloseOperationResult['operationId'],
	{ generation, status, target }: CloseObservation,
): StoppedTabClose | null {
	const stop = resumableStop(status);
	return stop === null || target.kind !== 'tab'
		? null
		: { closeRequestId: operationId, generation, stop, target };
}

/**
 * Reads an accepted close by its original identity, which outlives the target it closed.
 * `onProgressing` hears each read that finds the close still on its way.
 */
async function observeClose(
	session: CloseSession,
	operationId: CloseOperationResult['operationId'],
	deadline: number,
	onProgressing?: () => void,
): Promise<ObservedClose> {
	const unobserved = { result: failed('status-unavailable'), stopped: null };
	for (;;) {
		const remaining = deadline - performance.now();
		if (remaining <= 0) return unobserved;
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
			return unobserved;
		}
		const observation = answer.response.outcome;
		if (observation.status.kind === 'closed') await session.published(observation.status.revision);
		const settled = settledStatus(observation.status);
		if (settled !== null) {
			return { result: settled, stopped: stoppedTabClose(operationId, observation) };
		}
		onProgressing?.();
		await delay(Math.min(OBSERVATION_INTERVAL_MILLISECONDS, remaining));
	}
}

/**
 * Hands one close over and answers what the Server said about the target as shown. A newer
 * revision that left the target untouched is no refusal of it, so the close is handed over again.
 * A conditional close's question that described a target that has since moved on is asked again
 * once, since it answers nothing shown.
 */
async function submitShownClose(
	session: CloseSession,
	context: CloseContext,
	action: ServerWorkbenchAction | null,
	factsOf: (snapshot: WorkbenchSnapshot) => string,
): Promise<CloseSubmission> {
	let expected = factsOf(session.snapshot);
	let repeatedOnChange = false;
	for (let attempt = 0; ; attempt += 1) {
		const submission = await submitClose(session, context, action, factsOf, expected);
		if (submission.kind === 'ended' || submission.outcome.kind !== 'rejected') return submission;
		const { outcome } = submission;
		const current = factsOf(session.snapshot);
		if (outcome.reason === 'revision-mismatch') {
			if (current !== expected || attempt === STALE_RETRY_LIMIT) {
				return { kind: 'ended', result: SCOPE_CHANGED };
			}
		} else if (outcome.reason !== 'confirmation-required' || current === expected) {
			return submission;
		} else if (repeatedOnChange) {
			// The reply described a target that has moved on twice, so it answers nothing shown.
			return { kind: 'ended', result: SCOPE_CHANGED };
		} else {
			repeatedOnChange = true;
			expected = current;
		}
	}
}

/** Leaves a Tab out once, and shows it again once, through `leave`. */
function tabLeaving(
	leave: ServerTabLeave,
	{ tabId, terminalContextId }: TabTarget,
	inSequence: boolean,
): { readonly leave: () => void; readonly stay: () => void } {
	let stay: (() => void) | undefined;
	return {
		leave: () => {
			stay ??= leave.leave(terminalContextId, tabId, inSequence);
		},
		stay: () => {
			stay?.();
		},
	};
}

/**
 * Follows a Tab's accepted close to its end. As the TUI leaves a Tab out while its close
 * progresses, the window leaves it out until the close settles: a completed close has removed it
 * by then, and any other end shows it again with its report, keeping a stop Resume may continue.
 * A Close of a Tab whose close ended unsettled joins that close while the Server holds it open, so,
 * as the TUI keeps that close's last status, the Tab leaves only once a read finds the close on its
 * way again; until then it stays and reports the same stop.
 */
async function followTabClose(
	session: CloseSession,
	follow: TabCloseFollow,
	target: TabTarget,
	closeRequestId: CloseOperationResult['operationId'],
	deadline: number,
	inSequence = false,
): Promise<ActionResult<void>> {
	const { tabId, terminalContextId } = target;
	const leaving = tabLeaving(follow.leave, target, inSequence);
	const joined = follow.unsettled(terminalContextId, tabId)?.closeRequestId === closeRequestId;
	if (!joined) leaving.leave();
	try {
		const observed = await observeClose(session, closeRequestId, deadline, leaving.leave);
		follow.keep(
			terminalContextId,
			tabId,
			observed.result.kind === 'completed' ? null : { closeRequestId, stopped: observed.stopped },
		);
		return observed.result;
	} finally {
		leaving.stay();
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
	const submission = await submitShownClose(
		session,
		context,
		readServerWorkbenchAction(
			closure.type === 'workspace-close-if-idle'
				? { kind: 'workspace.close-if-idle', workspaceId: closure.workspaceId }
				: { kind: 'workspace.close', workspaceId: closure.workspaceId },
		),
		(snapshot) => closureFacts(closure, snapshot),
	);
	if (submission.kind === 'ended') return submission.result;
	const { outcome } = submission;
	if (outcome.kind !== 'workspace-close-accepted') return refused(outcome);
	const observed = await observeClose(
		session,
		outcome.closeRequestId,
		performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
	);
	return observed.result;
}

async function closeTabs(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'tabs-close' }>,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	const { terminalContextId } = closure;
	// As in the TUI, several Tabs close as one sequence, which keeps the selection where it is.
	const inSequence = closure.tabIds.length > 1;
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
		const result = await followTabClose(
			session,
			follow,
			{ tabId, terminalContextId },
			outcome.closeRequestId,
			deadline,
			inSequence,
		);
		if (result.kind !== 'completed') return result;
	}
	return { kind: 'completed' };
}

/** As the TUI asks, the Server closes the Tab unasked only when nothing runs in it. */
async function closeTabIfIdle(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'tab-close-if-idle' }>,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const submission = await submitShownClose(
		session,
		context,
		readServerWorkbenchAction({
			kind: 'tab.close-if-idle',
			tabId: closure.tabId,
			terminalContextId: closure.terminalContextId,
		}),
		(snapshot) => closureFacts(closure, snapshot),
	);
	if (submission.kind === 'ended') return submission.result;
	const { outcome } = submission;
	if (outcome.kind !== 'tab-close-accepted') return refused(outcome);
	return followTabClose(
		session,
		follow,
		closure,
		outcome.closeRequestId,
		performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
	);
}

/** What a Resume the Server did not run says; nothing it names has changed. */
function resumeRefused(
	outcome: Exclude<ResumeOutcome, { readonly kind: 'resumed' | 'stale' }>,
): ActionResult<void> {
	return {
		code: outcome.kind === 'denied' ? outcome.kind : outcome.reason,
		kind: 'blocked',
	};
}

/**
 * Continues exactly the close that stopped, under the attempt its status named, and never starts
 * another close in its place. A Stale answer means another attempt replaced that one, so this
 * window follows the close as it is now. Either answer puts the close back on its way, so, as in
 * the TUI, the Tab leaves at once.
 */
async function resumeTabClose(
	session: CloseSession,
	context: WorkbenchAuthorityContext,
	stopped: StoppedTabClose | null,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	if (stopped === null) return SCOPE_CHANGED;
	if (context.signal.aborted) return { kind: 'cancelled' };
	const { closeRequestId, generation, target } = stopped;
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	context.markCommitted();
	const answer = await session.ask(
		{
			closeRequestId,
			expectedAttemptGeneration: generation,
			kind: 'workbench.tab-close.resume',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
			tabId: target.tabId,
			terminalContextId: target.terminalContextId,
		},
		OBSERVATION_BUDGET_MILLISECONDS,
	);
	if (answer.kind === 'not-submitted') return NOT_SUBMITTED;
	if (answer.kind !== 'answered' || answer.response.kind !== 'workbench.tab-close.resume.result') {
		// It may have continued the close; a later Resume of the same attempt answers Stale then.
		return failed('outcome-unknown');
	}
	const { outcome } = answer.response;
	if (outcome.kind !== 'resumed' && outcome.kind !== 'stale') {
		// Whatever the Server refused here, a new Close finds the close as it is now.
		if (outcome.kind === 'rejected') follow.keep(target.terminalContextId, target.tabId, null);
		return resumeRefused(outcome);
	}
	follow.keep(target.terminalContextId, target.tabId, null);
	return followTabClose(session, follow, target, closeRequestId, deadline);
}

/**
 * A Panel settles when the Server accepts its close, and leaves with a later publication. A Tab's
 * last Panel takes the Tab with it, so that close is followed as a Tab's close is, keeping a stop
 * for Resume. As in the TUI, only a Tab's own close takes it out of the strip early.
 */
async function closePanel(
	session: CloseSession,
	context: CloseContext,
	closure: Extract<
		WorkbenchAuthorityClosure,
		{ readonly type: 'panel-close' | 'panel-close-if-idle' }
	>,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const { panelId, tabId, terminalContextId } = closure;
	const factsOf = (snapshot: WorkbenchSnapshot): string => closureFacts(closure, snapshot);
	const submission =
		closure.type === 'panel-close-if-idle'
			? await submitShownClose(
					session,
					context,
					readServerWorkbenchAction({
						kind: 'panel.close-if-idle',
						panelId,
						tabId,
						terminalContextId,
					}),
					factsOf,
				)
			: await submitClose(
					session,
					context,
					readServerWorkbenchAction({ kind: 'panel.close', panelId, tabId, terminalContextId }),
					factsOf,
					factsOf(session.snapshot),
				);
	if (submission.kind === 'ended') return submission.result;
	const { decidedFrom, outcome } = submission;
	if (outcome.kind !== 'panel-close-accepted') return refused(outcome);
	// The Server decided at this snapshot's revision, so its Tab held exactly these Panels.
	if (findTab(decidedFrom, terminalContextId, tabId)?.root.kind !== 'panel') {
		return { kind: 'completed' };
	}
	return followTabClose(
		session,
		{ ...follow, leave: NO_LEAVE },
		closure,
		outcome.closeRequestId,
		performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
	);
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

function tabKey(terminalContextId: string, tabId: string): string {
	return `${terminalContextId}:${tabId}`;
}

/**
 * Runs each close until its result is known and never repeats one that may have run. It keeps each
 * Tab close that ended without closing its Tab, with any stop an explicit Resume may continue,
 * while that Tab is still there. `leave` hears each Tab whose progressing close this follows, for
 * as long as it follows it.
 */
export function createServerWorkbenchClose(
	session: CloseSession,
	leave: ServerTabLeave = NO_LEAVE,
): Pick<WorkbenchAuthority, 'close' | 'stoppedTabClose' | 'watchClose'> {
	const unsettledTabCloses = new Map<string, UnsettledTabClose>();
	const follow: TabCloseFollow = {
		keep: (terminalContextId, tabId, unsettled) => {
			const key = tabKey(terminalContextId, tabId);
			unsettledTabCloses.delete(key);
			if (unsettled === null) return;
			unsettledTabCloses.set(key, unsettled);
			for (const oldest of unsettledTabCloses.keys()) {
				if (unsettledTabCloses.size <= MAXIMUM_UNSETTLED_TAB_CLOSES) break;
				unsettledTabCloses.delete(oldest);
			}
		},
		leave,
		unsettled: (terminalContextId, tabId) => {
			const key = tabKey(terminalContextId, tabId);
			const unsettled = unsettledTabCloses.get(key);
			if (unsettled === undefined) return null;
			const present = session.snapshot.terminalContexts
				.find((candidate) => candidate.terminalContextId === terminalContextId)
				?.tabs.some((candidate) => candidate.tabId === tabId);
			// Tab IDs never return, so a Tab that has gone takes its unsettled close with it.
			if (present !== true) {
				unsettledTabCloses.delete(key);
				return null;
			}
			return unsettled;
		},
	};
	const stoppedClose = (terminalContextId: string, tabId: string): StoppedTabClose | null =>
		follow.unsettled(terminalContextId, tabId)?.stopped ?? null;
	return {
		close: (context, closure) => {
			switch (closure.type) {
				case 'workspace-close':
				case 'workspace-close-if-idle':
					return closeWorkspace(session, closeContext(context), closure);
				case 'project-workspaces-close':
					return closeProject(session, context, closure);
				case 'tabs-close':
					return closeTabs(session, closeContext(context), closure, follow);
				case 'tab-close-if-idle':
					return closeTabIfIdle(session, closeContext(context), closure, follow);
				case 'panel-close':
				case 'panel-close-if-idle':
					return closePanel(session, closeContext(context), closure, follow);
				case 'tab-close-resume':
					return resumeTabClose(
						session,
						context,
						stoppedClose(closure.terminalContextId, closure.tabId),
						follow,
					);
			}
		},
		stoppedTabClose: (terminalContextId, tabId) =>
			stoppedClose(terminalContextId, tabId)?.stop ?? null,
		watchClose: (closure, onChanged) => {
			const expected = closureFacts(closure, session.snapshot);
			return session.subscribe((snapshot) => {
				if (closureFacts(closure, snapshot) !== expected) onChanged();
			});
		},
	};
}
