import type { ActionResult } from '@orchbee/actions';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityCloseContext,
	WorkbenchAuthorityClosure,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerLeave, ServerLeavingMember } from './ServerLeaving';
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
type GroupCloseResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.workspace-group-close.result' }
>;
type GroupCloseAction = Extract<
	Parameters<CloseSession['ask']>[0],
	{ readonly kind: 'workbench.workspace-group-close' }
>['action'];
type TabSetCloseResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.tab-set-close.result' }
>;
type TabSetCloseOutcome = TabSetCloseResult['outcome'];
type TabSetCloseReport = Extract<TabSetCloseOutcome, { readonly kind: 'available' }>;
type GroupCloseReport = Extract<GroupCloseResult['outcome'], { readonly kind: 'available' }>;
type TabSetCloseAction = Extract<
	Parameters<CloseSession['ask']>[0],
	{ readonly kind: 'workbench.tab-set-close' }
>['action'];
type TabsClosure = Extract<
	WorkbenchAuthorityClosure,
	{ readonly type: 'tabs-close' | 'tabs-close-if-idle' }
>;
/** A member of a retained operation that closes each member by its own Close. */
interface ClosingMember {
	readonly close: {
		readonly observation: { readonly status: CloseStatus } | null;
	} | null;
}
type CloseSubmission =
	| {
			readonly kind: 'answered';
			readonly outcome: ServerWorkbenchOutcome;
			/** The snapshot the close was handed over against, which the Server decided at. */
			readonly decidedFrom: WorkbenchSnapshot;
	  }
	| { readonly kind: 'ended'; readonly result: ActionResult<void> };
interface TabTarget {
	readonly tabId: string;
	readonly terminalContextId: string;
}

/** The Action's context, which also knows whether any step has been handed over yet. */
interface CloseContext extends WorkbenchAuthorityCloseContext {
	readonly handedOver: () => boolean;
}

/** The identity of a followed Tab close that ended without closing its Tab, which the Server keeps. */
type UnsettledTabClose = CloseOperationResult['operationId'];

/** What following a Tab's accepted close tells the window, and keeps for a later Close of the Tab. */
interface TabCloseFollow {
	readonly leave: ServerLeave;
	/** Keeps the Tab's followed close that ended without closing it, or forgets it with null. */
	keep(terminalContextId: string, tabId: string, unsettled: UnsettledTabClose | null): void;
	/** The followed close of the Tab that last ended without closing it, while the Tab is there. */
	unsettled(terminalContextId: string, tabId: string): UnsettledTabClose | null;
}

/**
 * How long this Client follows a close that still reads as running. The Server keeps one close
 * `closing` across its attempt and two automatic renewals of 30 s each, and a step admitted just
 * before an attempt's deadline may still settle after it, within the carrier's 20 s response wait
 * and 2 s frame I/O: at most about 156 s (the Server's terminal-tab-close reference, Automatic
 * renewal). A Tab set or Project close-all ends itself after 120 s. This bound sits above all of
 * them, so the window stops following only where the Server can no longer be closing.
 */
const OBSERVATION_BUDGET_MILLISECONDS = 180_000;
/** Physical settlement has no progress notification, so only a pending close is sampled. */
const OBSERVATION_INTERVAL_MILLISECONDS = 50;
/** A newer revision that left the target untouched is not a refusal of the target. */
const STALE_RETRY_LIMIT = 4;
/** Unsettled Tab closes kept for a later Close; one person rarely leaves more behind. */
const MAXIMUM_UNSETTLED_TAB_CLOSES = 16;

const NOT_SUBMITTED = { code: 'not-submitted', kind: 'blocked' } as const;
const SCOPE_CHANGED = { code: 'scope-changed', kind: 'blocked' } as const;
const NO_LEAVE: ServerLeave = { leave: () => () => undefined };
const CONFIRMATION_REQUIRED = { code: 'confirmation-required', kind: 'blocked' } as const;

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
		case 'tabs-close-if-idle':
			return JSON.stringify(
				closure.tabIds.map((tabId) => tabFacts(snapshot, closure.terminalContextId, tabId)),
			);
		case 'panel-close':
		case 'panel-close-if-idle':
			return tabFacts(snapshot, closure.terminalContextId, closure.tabId);
	}
}

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function closeContext(context: WorkbenchAuthorityCloseContext): CloseContext {
	let handedOver = false;
	return {
		handedOver: () => handedOver,
		markCommitted: () => {
			handedOver = true;
			context.markCommitted();
		},
		markLeft: context.markLeft,
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

/**
 * What a close status means here; null while the close is still under way. The Server retries and
 * renews a close by itself while it reads `closing`, so any parked status means it has given up.
 */
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

/**
 * Reads an accepted close by its original identity, which outlives the target it closed, for as
 * long as it reads as running, however often the Server renews its attempt meanwhile. It stops at
 * a settled status, a read that brings no status, or the deadline. `onProgressing` hears each read
 * that finds the close still on its way.
 */
async function observeClose(
	session: CloseSession,
	operationId: CloseOperationResult['operationId'],
	deadline: number,
	onProgressing?: () => void,
): Promise<ActionResult<void>> {
	const unobserved = failed('status-unavailable');
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
		if (settled !== null) return settled;
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

/** Leaves one member out once, and shows it again once, through `leave`; `onLeft` hears the leave. */
function memberLeaving(
	leave: ServerLeave,
	member: ServerLeavingMember,
	onLeft: (() => void) | undefined,
): { readonly leave: () => void; readonly stay: () => void } {
	let stay: (() => void) | undefined;
	return {
		leave: () => {
			if (stay !== undefined) return;
			stay = leave.leave([member]);
			onLeft?.();
		},
		stay: () => {
			stay?.();
		},
	};
}

/**
 * Follows a Tab's accepted close to its end. As the TUI leaves a Tab out while its close
 * progresses, the window leaves it out until the close settles: a completed close has removed it
 * by then, and any other end shows it again with its report. A Close of a Tab whose close ended
 * unsettled joins that close while the Server holds it open, and the Server may renew it or leave
 * it as it was, so the Tab leaves only once a read finds the close running; until then it stays,
 * and a close that does not run again reports its end again. `markLeft` hears the Tab leave.
 */
async function followTabClose(
	session: CloseSession,
	follow: TabCloseFollow,
	target: TabTarget,
	closeRequestId: CloseOperationResult['operationId'],
	deadline: number,
	markLeft: (() => void) | undefined,
): Promise<ActionResult<void>> {
	const { tabId, terminalContextId } = target;
	const leaving = memberLeaving(follow.leave, { kind: 'tab', tabId, terminalContextId }, markLeft);
	const joined = follow.unsettled(terminalContextId, tabId) === closeRequestId;
	if (!joined) leaving.leave();
	try {
		const result = await observeClose(session, closeRequestId, deadline, leaving.leave);
		follow.keep(terminalContextId, tabId, result.kind === 'completed' ? null : closeRequestId);
		return result;
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
	follow: TabCloseFollow,
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
	// The Server keeps the Workspace until its removal publishes, so the window leaves it out from here.
	const stay = follow.leave.leave([{ kind: 'workspace', workspaceId: closure.workspaceId }]);
	context.markLeft?.();
	try {
		return await observeClose(
			session,
			outcome.closeRequestId,
			performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
		);
	} finally {
		stay();
	}
}

/** One Tab is no set: it closes as its own Close, which asks first in the if-idle form. */
async function closeTabs(
	session: CloseSession,
	context: CloseContext,
	closure: TabsClosure,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const [tabId, ...others] = closure.tabIds;
	// The window names only Tabs it shows, so a set with none names what has gone since.
	if (tabId === undefined) return SCOPE_CHANGED;
	if (others.length > 0) return closeTabSet(session, context, closure, follow);
	const { terminalContextId } = closure;
	if (closure.type === 'tabs-close-if-idle') {
		return closeTabIfIdle(session, context, { tabId, terminalContextId }, follow);
	}
	const submission = await submitClose(
		session,
		context,
		readServerWorkbenchAction({ kind: 'tab.close', tabId, terminalContextId }),
		(snapshot) => tabFacts(snapshot, terminalContextId, tabId),
		tabFacts(session.snapshot, terminalContextId, tabId),
	);
	if (submission.kind === 'ended') return submission.result;
	const { outcome } = submission;
	if (outcome.kind !== 'tab-close-accepted') return refused(outcome);
	return followTabClose(
		session,
		follow,
		{ tabId, terminalContextId },
		outcome.closeRequestId,
		performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
		context.markLeft,
	);
}

function tabSetOutcome(answer: ServerWorkbenchAnswer): TabSetCloseOutcome | null {
	return answer.kind === 'answered' && answer.response.kind === 'workbench.tab-set-close.result'
		? answer.response.outcome
		: null;
}

function askTabSetClose(
	session: CloseSession,
	action: TabSetCloseAction,
	requestId: TabSetCloseResult['requestId'],
	timeoutMilliseconds?: number,
): Promise<ServerWorkbenchAnswer> {
	return session.ask(
		{
			action,
			kind: 'workbench.tab-set-close',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: session.snapshot.sessionId,
		},
		timeoutMilliseconds,
	);
}

/** A set the Server did not take retains nothing, so every member is as it was. */
function tabSetRefused(
	outcome: Exclude<TabSetCloseOutcome, TabSetCloseReport>,
): ActionResult<void> {
	switch (outcome.kind) {
		case 'confirmation-required':
			return CONFIRMATION_REQUIRED;
		case 'rejected':
		case 'unavailable':
			return { code: outcome.reason, kind: 'blocked' };
		case 'conflict':
		case 'denied':
		case 'not-found':
			return { code: outcome.kind, kind: 'blocked' };
	}
}

/** The members as the snapshot names them, in the order given; null when one is not there. */
function heldTabSet(snapshot: WorkbenchSnapshot, closure: TabsClosure) {
	const context = snapshot.terminalContexts.find(
		(candidate) => candidate.terminalContextId === closure.terminalContextId,
	);
	const tabIds = closure.tabIds.flatMap((tabId) => {
		const tab = context?.tabs.find((candidate) => candidate.tabId === tabId);
		return tab === undefined ? [] : [tab.tabId];
	});
	return context === undefined || tabIds.length !== closure.tabIds.length
		? null
		: { tabIds, terminalContextId: context.terminalContextId };
}

/**
 * Keeps the close of the member a stopped set left: the Server leaves that member's own Close
 * retained, so a new Close of the Tab joins it.
 */
function keepStoppedMember(
	follow: TabCloseFollow,
	{ members, terminalContextId }: TabSetCloseReport,
): void {
	const member = members.find(
		(candidate) => !candidate.absent && candidate.close?.observation?.status.kind !== 'closed',
	);
	const child = member?.close;
	if (member === undefined || child === null || child === undefined) return;
	follow.keep(terminalContextId, member.tabId, child.requestId);
}

/**
 * Several Tabs close as one Server operation, which closes the members one at a time, each by its
 * own Close, and stops at the first that fails; this Client starts it once and reads it. A Start a
 * newer revision turned away goes again while every member is still as shown. As the TUI does,
 * every member leaves the strip once the Server accepts the set, so a selected member gives way at
 * once to the next Tab outside the set, else the previous one, and a focus change stops nothing.
 * The members the Server leaves open come back with its report, and the selection stays where it
 * moved.
 */
async function closeTabSet(
	session: CloseSession,
	context: CloseContext,
	closure: TabsClosure,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const deadline = performance.now() + OBSERVATION_BUDGET_MILLISECONDS;
	const factsOf = (snapshot: WorkbenchSnapshot): string => closureFacts(closure, snapshot);
	const expected = factsOf(session.snapshot);
	const condition = closure.type === 'tabs-close-if-idle' ? 'if-idle' : 'confirmed';
	let operationId = createWorkbenchRequestId();
	let outcome: TabSetCloseOutcome | null = null;
	for (let attempt = 0; ; attempt += 1) {
		const { snapshot } = session;
		const held = heldTabSet(snapshot, closure);
		if (held === null || factsOf(snapshot) !== expected) return SCOPE_CHANGED;
		// Once a step has been handed over, cancelling can no longer say nothing happened.
		if (context.signal.aborted) return context.handedOver() ? NOT_SUBMITTED : { kind: 'cancelled' };
		context.markCommitted();
		const started = await askTabSetClose(
			session,
			{
				condition,
				expectedRevision: snapshot.revision,
				kind: 'start',
				operationId,
				...held,
			},
			operationId,
		);
		if (started.kind === 'not-submitted') return NOT_SUBMITTED;
		outcome = tabSetOutcome(started);
		if (outcome === null) return failed('outcome-unknown', operationId);
		if (outcome.kind !== 'rejected' || outcome.reason !== 'revision-mismatch') break;
		// A newer revision that left every member as shown is no refusal of the set.
		await session.published(outcome.revision);
		if (factsOf(session.snapshot) !== expected || attempt === STALE_RETRY_LIMIT) {
			return SCOPE_CHANGED;
		}
		operationId = createWorkbenchRequestId();
	}
	if (outcome.kind !== 'available') return tabSetRefused(outcome);
	let report: TabSetCloseReport = outcome;
	// The closed members leave with their publications before the others come back.
	const published = async (): Promise<void> => {
		const revision = lastClosedRevision(report.members);
		if (revision !== null) await session.published(revision);
	};
	const stay = follow.leave.leave(
		closure.tabIds.map(
			(tabId): ServerLeavingMember => ({
				kind: 'tab',
				tabId,
				terminalContextId: closure.terminalContextId,
			}),
		),
	);
	try {
		for (;;) {
			const { status } = report;
			if (status.kind === 'failed') {
				await published();
				keepStoppedMember(follow, report);
				return failed(
					status.reason === 'outcome-unknown' ? 'outcome-unknown' : 'close-stopped',
					operationId,
				);
			}
			if (status.kind === 'closed') {
				await published();
				return { kind: 'completed' };
			}
			const remaining = deadline - performance.now();
			if (remaining <= 0) return failed('status-unavailable', operationId);
			await delay(Math.min(OBSERVATION_INTERVAL_MILLISECONDS, remaining));
			const next = tabSetOutcome(
				await askTabSetClose(
					session,
					{ kind: 'read', operationId },
					createWorkbenchRequestId(),
					remaining,
				),
			);
			if (next?.kind !== 'available') return failed('status-unavailable', operationId);
			report = next;
		}
	} finally {
		stay();
	}
}

/** As the TUI asks, the Server closes the Tab unasked only when nothing runs in it. */
async function closeTabIfIdle(
	session: CloseSession,
	context: CloseContext,
	target: TabTarget,
	follow: TabCloseFollow,
): Promise<ActionResult<void>> {
	const { tabId, terminalContextId } = target;
	const submission = await submitShownClose(
		session,
		context,
		readServerWorkbenchAction({ kind: 'tab.close-if-idle', tabId, terminalContextId }),
		(snapshot) => tabFacts(snapshot, terminalContextId, tabId),
	);
	if (submission.kind === 'ended') return submission.result;
	const { outcome } = submission;
	if (outcome.kind !== 'tab-close-accepted') return refused(outcome);
	return followTabClose(
		session,
		follow,
		target,
		outcome.closeRequestId,
		performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
		context.markLeft,
	);
}

/**
 * Follows the accepted close of one Panel of several. The Server removes such a Panel only as its
 * close completes, so the window leaves it out from the acceptance until the close settles: a
 * completed close has removed it by then, and any other end shows it again with its report.
 */
async function followPanelClose(
	session: CloseSession,
	leave: ServerLeave,
	target: { readonly panelId: string; readonly tabId: string; readonly terminalContextId: string },
	closeRequestId: CloseOperationResult['operationId'],
): Promise<ActionResult<void>> {
	const { panelId, tabId, terminalContextId } = target;
	const stay = leave.leave([{ kind: 'panel', panelId, tabId, terminalContextId }]);
	try {
		return await observeClose(
			session,
			closeRequestId,
			performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
		);
	} finally {
		stay();
	}
}

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
	if (findTab(decidedFrom, terminalContextId, tabId)?.root.kind === 'panel') {
		// The last Panel is its whole Tab, so its close follows as the close of that Tab.
		return followTabClose(
			session,
			follow,
			closure,
			outcome.closeRequestId,
			performance.now() + OBSERVATION_BUDGET_MILLISECONDS,
			context.markLeft,
		);
	}
	return followPanelClose(session, follow.leave, closure, outcome.closeRequestId);
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
function lastClosedRevision(members: readonly ClosingMember[]): string | null {
	let last: string | null = null;
	for (const member of members) {
		const status = member.close?.observation?.status;
		if (status?.kind === 'closed' && (last === null || BigInt(status.revision) > BigInt(last))) {
			last = status.revision;
		}
	}
	return last;
}

/**
 * The Server owns the sequence and captures the members; this Client only watches it settle. As the
 * TUI hides them while the group close is pending or closed, the members leave once it is accepted
 * and not already stopped.
 */
async function closeProject(
	session: CloseSession,
	context: WorkbenchAuthorityCloseContext,
	closure: Extract<WorkbenchAuthorityClosure, { readonly type: 'project-workspaces-close' }>,
	leave: ServerLeave,
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
	const report = groupOutcome(started);
	if (report === null) return failed('outcome-unknown', operationId);
	if (report.kind !== 'available') {
		return {
			code: report.kind === 'unavailable' ? report.reason : report.kind,
			kind: 'blocked',
		};
	}
	// A close that stopped before anything left takes nothing out of view.
	if (report.status.kind === 'failed') {
		return watchProjectClose(session, operationId, report, deadline);
	}
	// The Server accepted the Project's close and captured its members, so they leave from here.
	const stay = leave.leave(
		report.members.map(
			({ workspaceId }): ServerLeavingMember => ({ kind: 'workspace', workspaceId }),
		),
	);
	context.markLeft?.();
	try {
		return await watchProjectClose(session, operationId, report, deadline);
	} finally {
		stay();
	}
}

/** Watches an accepted Project close until it settles, as the Server settles its members. */
async function watchProjectClose(
	session: CloseSession,
	operationId: ReturnType<typeof createWorkbenchRequestId>,
	report: GroupCloseReport,
	deadline: number,
): Promise<ActionResult<void>> {
	let outcome = report;
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
 * Tab close that ended without closing its Tab while that Tab is still there, so a new Close of the
 * Tab is known to join it. `leave` hears each Tab whose progressing close this follows, for as long
 * as it follows it.
 */
export function createServerWorkbenchClose(
	session: CloseSession,
	leave: ServerLeave = NO_LEAVE,
): Pick<WorkbenchAuthority, 'close' | 'watchClose'> {
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
	return {
		close: (context, closure) => {
			switch (closure.type) {
				case 'workspace-close':
				case 'workspace-close-if-idle':
					return closeWorkspace(session, closeContext(context), closure, follow);
				case 'project-workspaces-close':
					return closeProject(session, context, closure, follow.leave);
				case 'tabs-close':
				case 'tabs-close-if-idle':
					return closeTabs(session, closeContext(context), closure, follow);
				case 'panel-close':
				case 'panel-close-if-idle':
					return closePanel(session, closeContext(context), closure, follow);
			}
		},
		watchClose: (closure, onChanged) => {
			const expected = closureFacts(closure, session.snapshot);
			return session.subscribe((snapshot) => {
				if (closureFacts(closure, snapshot) !== expected) onChanged();
			});
		},
	};
}
