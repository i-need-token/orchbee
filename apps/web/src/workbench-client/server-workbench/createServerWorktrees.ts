import type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeDiscovery,
	WorkbenchWorktreeDiscoveryResult,
	WorkbenchWorktreeFailureCause,
	WorkbenchWorktreeOutcome,
	WorkbenchWorktreeRemoval,
	WorkbenchWorktrees,
	WorkbenchWorktreeStoppedRemoval,
} from '@orchbee/workbench';
import {
	parseWorkbenchRequest,
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchPanelNode,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from './ServerWorkbenchSession';
import { DEFAULT_TERMINAL_GEOMETRY } from './workspace-launch';
import { sameDirectorySpelling, worktreeDestination } from './worktree-paths';

type WorktreeSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;
type AskedRequest = Parameters<WorktreeSession['ask']>[0];
/** A Workspace source before the protocol admits it; every request is parsed before it is sent. */
type WorktreeSource =
	| { readonly kind: 'workspace'; readonly workspaceId: string }
	| { readonly kind: 'workspace-directory'; readonly path: string; readonly workspaceId: string };
type WorktreeOperationOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.worktree.operation.result' }
>['outcome'];
type WorktreeFoundListing = Extract<
	Extract<WorkbenchResponse, { readonly kind: 'workbench.worktree.query.result' }>['outcome'],
	{ readonly kind: 'found' }
>;
type WorktreeOperationFailure = Extract<
	Extract<WorktreeOperationOutcome, { readonly kind: 'available' }>['status'],
	{ readonly kind: 'failed' }
>['reason'];

/** What a discovery reported, with the source it read and the repository it found. */
interface DiscoveryFacts {
	readonly discovery: WorkbenchWorktreeDiscovery;
	readonly repositoryRoot: string;
	readonly source: WorktreeSource;
}

type DiscoveryReading =
	| { readonly kind: 'discovered'; readonly facts: DiscoveryFacts }
	| {
			readonly kind: 'failed';
			readonly reason: Extract<
				WorkbenchWorktreeDiscoveryResult,
				{ readonly kind: 'failed' }
			>['reason'];
	  };

/** What a settled operation must report to be the one this Client asked for. */
type Expectation =
	| { readonly kind: 'opened'; readonly path: string | null }
	| { readonly kind: 'removed'; readonly path: string; readonly workspaceId: string };

interface FollowedOperation {
	readonly attempt: number;
	readonly expectation: Expectation;
	readonly operationId: string;
}

/** A removal kept for Resume: the operation to continue, why it stopped, and its Workspace. */
interface StoppedOperation {
	readonly cause: WorkbenchWorktreeFailureCause;
	readonly operation: FollowedOperation;
	readonly workspaceId: string;
}

/** The native Clients' discovery deadline, which covers the directory sample and Git's listing. */
const DISCOVERY_MILLISECONDS = 10_000;
const CURRENT_DIRECTORY_MILLISECONDS = 5_000;
const CONFIGURATION_MILLISECONDS = 5_000;
/** The carrier's response budget for a request that starts or resumes an operation. */
const START_MILLISECONDS = 20_000;
/** How long the native Clients follow an operation before its outcome counts as unknown. */
const OPERATION_MILLISECONDS = 120_000;
const FIRST_READ_MILLISECONDS = 250;
const LAST_READ_MILLISECONDS = 2_000;
const MAXIMUM_OPERATION_ATTEMPTS = 64;
/** Stopped removals kept for Resume; one person rarely leaves more than one behind. */
const MAXIMUM_STOPPED_REMOVALS = 16;

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/** Values the Client relays can fall outside what the protocol admits, which is a refusal. */
function readRequest(input: Readonly<Record<string, unknown>>): AskedRequest | null {
	try {
		return parseWorkbenchRequest(input);
	} catch {
		return null;
	}
}

function shortBranch(branch: string | null): string | null {
	return branch?.replace(/^refs\/heads\//u, '') ?? null;
}

function primaryDirectory(
	snapshot: WorkbenchSnapshot,
	workspaceId: string,
): WorkbenchSnapshot['navigation']['directories'][number] | null {
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === workspaceId,
	);
	return (
		snapshot.navigation.directories.find(
			(directory) => directory.directoryId === workspace?.primaryDirectoryId,
		) ?? null
	);
}

function boundTerminals(node: WorkbenchPanelNode): number {
	if (node.kind === 'split') {
		return node.children.reduce((count, child) => count + boundTerminals(child.node), 0);
	}
	return [node.primaryPresentation, ...node.childPresentations].filter(
		(presentation) => presentation.binding !== null,
	).length;
}

async function currentDirectory(
	session: WorktreeSession,
	workspaceId: string,
): Promise<string | null> {
	const requestId = createWorkbenchRequestId();
	const request = readRequest({
		kind: 'workbench.current-directory',
		protocolVersion: WORKBENCH_PROTOCOL_VERSION,
		requestId,
		sessionId: session.snapshot.sessionId,
		workspaceId,
	});
	if (request === null) return null;
	const answer = await session.ask(request, CURRENT_DIRECTORY_MILLISECONDS);
	return answer.kind === 'answered' &&
		answer.response.kind === 'workbench.current-directory.result' &&
		answer.response.requestId === requestId &&
		answer.response.outcome.kind === 'observed'
		? answer.response.outcome.directory
		: null;
}

/**
 * Where the TUI reads a Workspace's repository: where its terminal stands when that differs from the
 * Workspace directory, otherwise the Workspace itself. Without a sample, only a Workspace with a
 * directory has a source.
 */
function workspaceSource(
	snapshot: WorkbenchSnapshot,
	workspaceId: string,
	observed: string | null,
): WorktreeSource | null {
	const directory = primaryDirectory(snapshot, workspaceId);
	if (
		observed !== null &&
		(directory === null || !sameDirectorySpelling(directory.path, observed))
	) {
		return { kind: 'workspace-directory', path: observed, workspaceId };
	}
	return directory === null && observed === null ? null : { kind: 'workspace', workspaceId };
}

/**
 * The Workspace's own directory when Git lists it as a linked, non-bare checkout associated with this
 * Workspace on the Worker that read the listing; the TUI offers removal under the same proof.
 */
function removalTarget(
	snapshot: WorkbenchSnapshot,
	workspaceId: string,
	listing: WorktreeFoundListing,
): WorkbenchWorktreeRemoval | null {
	const directory = primaryDirectory(snapshot, workspaceId);
	if (directory === null || directory.workerId !== listing.workerId) return null;
	const entry = listing.entries.find(
		(candidate) =>
			candidate.linked &&
			!candidate.checkout.bare &&
			sameDirectorySpelling(directory.path, candidate.checkout.worktreePath) &&
			candidate.workspaces.some((workspace) => workspace.workspaceId === workspaceId),
	);
	if (entry === undefined) return null;
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === workspaceId,
	);
	const context = snapshot.terminalContexts.find(
		(candidate) => candidate.terminalContextId === workspace?.terminalContextId,
	);
	return {
		branch: shortBranch(entry.checkout.branch),
		path: directory.path,
		terminalCount: (context?.tabs ?? []).reduce(
			(count, tab) => count + boundTerminals(tab.root),
			0,
		),
	};
}

function failureCause(reason: WorktreeOperationFailure): WorkbenchWorktreeFailureCause {
	if (reason.kind === 'git') {
		switch (reason.error) {
			case 'checkout-not-found':
				return 'checkout-not-found';
			case 'dirty':
				return 'dirty';
			case 'git-failed':
				return 'git-failed';
			case 'io':
				return 'directory-in-use';
			case 'main-checkout':
				return 'main-checkout';
			default:
				return 'git';
		}
	}
	switch (reason.kind) {
		case 'checkout-in-use':
		case 'close-incomplete':
		case 'session-stopping':
		case 'source-changed':
		case 'unsupported':
			return reason.kind;
		case 'worker-busy':
		case 'worker-capacity':
		case 'worker-unavailable':
			return 'worker-unavailable';
		default:
			return 'internal';
	}
}

function failed(cause: WorkbenchWorktreeFailureCause): WorkbenchWorktreeOutcome {
	return { kind: 'failed', cause, resumable: null };
}

/**
 * The Git Worktree port of one Server Session. Discovery and operations use the native Clients'
 * requests and rules, so every Client offers, starts and follows the same operations.
 */
export function createServerWorktrees(session: WorktreeSession): WorkbenchWorktrees {
	/** The latest discovery of each Workspace, which its commands act on. */
	const discoveries = new Map<string, DiscoveryFacts>();
	const readings = new Map<string, number>();
	/** Oldest first, so the newest of a Workspace is the last one listed. */
	const stoppedRemovals = new Map<string, StoppedOperation>();

	function stop(stopped: StoppedOperation): void {
		const { operationId } = stopped.operation;
		stoppedRemovals.delete(operationId);
		stoppedRemovals.set(operationId, stopped);
		for (const kept of stoppedRemovals.keys()) {
			if (stoppedRemovals.size <= MAXIMUM_STOPPED_REMOVALS) break;
			stoppedRemovals.delete(kept);
		}
	}

	function stoppedRemovalOf(workspaceId: string): WorkbenchWorktreeStoppedRemoval | null {
		let latest: StoppedOperation | null = null;
		for (const stopped of stoppedRemovals.values()) {
			if (stopped.workspaceId === workspaceId) latest = stopped;
		}
		return latest === null
			? null
			: { cause: latest.cause, operationId: latest.operation.operationId };
	}

	/** What a settled answer means, or null while the operation is still running. */
	function settled(
		outcome: WorktreeOperationOutcome,
		operation: FollowedOperation,
		decisive: boolean,
	): WorkbenchWorktreeOutcome | null {
		switch (outcome.kind) {
			case 'not-found':
				return { kind: 'unknown', operationId: operation.operationId };
			case 'conflict':
				return failed('internal');
			case 'denied':
				return failed('denied');
			case 'unavailable':
				// A read the Server could not serve now says nothing about the operation itself.
				if (!decisive) return null;
				if (outcome.reason === 'unsupported') return failed('unsupported');
				return failed(
					outcome.reason === 'source-unavailable' ? 'source-changed' : 'worker-unavailable',
				);
			case 'available':
				break;
		}
		if (outcome.attempt !== operation.attempt) return null;
		const { expectation } = operation;
		const status = outcome.status;
		switch (status.kind) {
			case 'pending':
				return null;
			case 'opened':
				// Git names the checkout in its own spelling.
				return expectation.kind === 'opened' &&
					(expectation.path === null ||
						sameDirectorySpelling(expectation.path, status.checkout.path))
					? { kind: 'opened', workspaceId: status.workspaceId }
					: failed('source-changed');
			case 'removed':
				return expectation.kind === 'removed' &&
					sameDirectorySpelling(expectation.path, status.path) &&
					expectation.workspaceId === status.workspaceId
					? { kind: 'removed' }
					: failed('source-changed');
			case 'failed': {
				if (
					status.reason.kind === 'physical-unknown' ||
					status.reason.kind === 'transport-unknown'
				) {
					return { kind: 'unknown', operationId: operation.operationId };
				}
				const cause = failureCause(status.reason);
				if (
					expectation.kind !== 'removed' ||
					!status.resumable ||
					outcome.attempt >= MAXIMUM_OPERATION_ATTEMPTS ||
					(status.progress.restoration?.unknown ?? 0) !== 0
				) {
					return failed(cause);
				}
				stop({ cause, operation, workspaceId: expectation.workspaceId });
				const removal: WorkbenchWorktreeStoppedRemoval = {
					cause,
					operationId: operation.operationId,
				};
				return { kind: 'failed', cause, resumable: removal };
			}
		}
	}

	async function read(operationId: string, milliseconds: number): Promise<ServerWorkbenchAnswer> {
		const request = readRequest({
			kind: 'workbench.worktree.operation',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
			action: { kind: 'read', operationId },
		});
		return request === null ? { kind: 'not-submitted' } : session.ask(request, milliseconds);
	}

	/**
	 * Follows an operation from the answer that started it, reading at the TUI's backoff until it
	 * settles. A started operation is never started again, so what cannot be observed is unknown.
	 */
	async function follow(
		operation: FollowedOperation,
		started: ServerWorkbenchAnswer,
	): Promise<WorkbenchWorktreeOutcome> {
		if (started.kind === 'not-submitted') return failed('not-submitted');
		const deadline = performance.now() + OPERATION_MILLISECONDS;
		let answer: ServerWorkbenchAnswer = started;
		let decisive = true;
		let wait = FIRST_READ_MILLISECONDS;
		for (;;) {
			if (
				answer.kind === 'answered' &&
				answer.response.kind === 'workbench.worktree.operation.result' &&
				answer.response.operationId === operation.operationId
			) {
				const outcome = settled(answer.response.outcome, operation, decisive);
				if (outcome !== null) return outcome;
			}
			const remaining = deadline - performance.now() - wait;
			if (remaining <= 0) return { kind: 'unknown', operationId: operation.operationId };
			await delay(wait);
			wait = Math.min(wait * 2, LAST_READ_MILLISECONDS);
			decisive = false;
			answer = await read(operation.operationId, Math.min(remaining, START_MILLISECONDS));
		}
	}

	async function start(
		action: Readonly<Record<string, unknown>>,
		operation: FollowedOperation,
	): Promise<WorkbenchWorktreeOutcome> {
		const request = readRequest({
			kind: 'workbench.worktree.operation',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
			action,
		});
		if (request === null) return failed('internal');
		return follow(operation, await session.ask(request, START_MILLISECONDS));
	}

	async function worktreesDirectory(): Promise<{ readonly root: string | null } | null> {
		const requestId = createWorkbenchRequestId();
		const request = readRequest({
			kind: 'workbench.configuration',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: session.snapshot.sessionId,
			action: 'current',
		});
		if (request === null) return null;
		const answer = await session.ask(request, CONFIGURATION_MILLISECONDS);
		if (
			answer.kind !== 'answered' ||
			answer.response.kind !== 'workbench.configuration.result' ||
			answer.response.requestId !== requestId ||
			answer.response.outcome.kind === 'failed'
		) {
			return null;
		}
		return { root: answer.response.outcome.snapshot.worktreesDirectory };
	}

	async function run(command: WorkbenchWorktreeCommand): Promise<WorkbenchWorktreeOutcome> {
		if (command.kind === 'resume') {
			const stopped = stoppedRemovals.get(command.operationId);
			if (stopped === undefined) return failed('source-changed');
			const { operation } = stopped;
			const outcome = await start(
				{ kind: 'resume', operationId: operation.operationId, expectedAttempt: operation.attempt },
				{ ...operation, attempt: operation.attempt + 1 },
			);
			// A Resume that never left keeps the removal resumable; any other end replaced or retired it.
			if (
				!(outcome.kind === 'failed' && outcome.cause === 'not-submitted') &&
				(outcome.kind !== 'failed' || outcome.resumable === null)
			) {
				stoppedRemovals.delete(command.operationId);
			}
			return outcome;
		}
		const facts = discoveries.get(command.workspaceId);
		if (facts === undefined) return failed('source-changed');
		const operationId = createWorkbenchRequestId();
		const launch = {
			source: facts.source,
			geometry: DEFAULT_TERMINAL_GEOMETRY,
			trustRepository: false,
		};
		switch (command.kind) {
			case 'create': {
				const configured = await worktreesDirectory();
				if (configured === null) return failed('configuration-unavailable');
				const path =
					configured.root === null
						? undefined
						: worktreeDestination(configured.root, facts.repositoryRoot, `worktree/${operationId}`);
				if (path === null) return failed('internal');
				return start(
					{
						kind: 'start',
						operationId,
						projectId: null,
						intent: {
							kind: 'create',
							...launch,
							label: command.label,
							...(path === undefined ? {} : { path }),
						},
					},
					{ attempt: 1, expectation: { kind: 'opened', path: null }, operationId },
				);
			}
			case 'open': {
				const checkout = facts.discovery.checkouts.find((candidate) =>
					sameDirectorySpelling(candidate.path, command.path),
				);
				if (checkout === undefined) return failed('source-changed');
				return start(
					{
						kind: 'start',
						operationId,
						projectId: null,
						intent: { kind: 'open', ...launch, selector: { kind: 'path', path: checkout.path } },
					},
					{ attempt: 1, expectation: { kind: 'opened', path: checkout.path }, operationId },
				);
			}
			case 'remove': {
				const { workspaceId } = command;
				const { removal } = facts.discovery;
				const directory = primaryDirectory(session.snapshot, workspaceId);
				// What the person confirmed must still be what Git offered and what the Workspace stands in.
				if (
					removal === null ||
					directory === null ||
					!sameDirectorySpelling(removal.path, command.path) ||
					!sameDirectorySpelling(directory.path, removal.path)
				) {
					return failed('source-changed');
				}
				return start(
					{
						kind: 'start',
						operationId,
						projectId: null,
						intent: { kind: 'remove', workspaceId, force: false, trustRepository: false },
					},
					{
						attempt: 1,
						expectation: { kind: 'removed', path: removal.path, workspaceId },
						operationId,
					},
				);
			}
		}
	}

	async function listCheckouts(workspaceId: string): Promise<DiscoveryReading> {
		const unavailable: DiscoveryReading = { kind: 'failed', reason: 'unavailable' };
		const deadline = performance.now() + DISCOVERY_MILLISECONDS;
		const observed = await currentDirectory(session, workspaceId);
		const source = workspaceSource(session.snapshot, workspaceId, observed);
		const remaining = deadline - performance.now();
		if (source === null || remaining <= 0) return unavailable;
		const requestId = createWorkbenchRequestId();
		const request = readRequest({
			kind: 'workbench.worktree.query',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: session.snapshot.sessionId,
			source,
			trustRepository: false,
		});
		if (request === null) return unavailable;
		const answer = await session.ask(request, remaining);
		if (
			answer.kind !== 'answered' ||
			answer.response.kind !== 'workbench.worktree.query.result' ||
			answer.response.requestId !== requestId
		) {
			return unavailable;
		}
		const listing = answer.response.outcome;
		if (listing.kind === 'failed') {
			// Only a failed probe tells the person something to act on: there is no repository it trusts.
			return listing.reason.kind === 'git-failed' && listing.reason.stage === 'probe'
				? { kind: 'failed', reason: 'not-repository' }
				: unavailable;
		}
		const discovery: WorkbenchWorktreeDiscovery = {
			checkouts: listing.entries
				.filter(
					(entry) =>
						!entry.checkout.bare && entry.checkout.worktreePath !== listing.source.checkoutPath,
				)
				.map((entry) => ({
					branch: shortBranch(entry.checkout.branch),
					path: entry.checkout.worktreePath,
				})),
			removal: removalTarget(session.snapshot, workspaceId, listing),
			workspaceId,
		};
		return {
			kind: 'discovered',
			facts: { discovery, repositoryRoot: listing.source.repositoryRoot, source },
		};
	}

	return {
		async discover(workspaceId: string): Promise<WorkbenchWorktreeDiscoveryResult> {
			const reading = (readings.get(workspaceId) ?? 0) + 1;
			readings.set(workspaceId, reading);
			const result = await listCheckouts(workspaceId);
			// Only the newest read speaks for the Workspace, as only it reaches the menu.
			if (readings.get(workspaceId) === reading) {
				if (result.kind === 'discovered') discoveries.set(workspaceId, result.facts);
				else discoveries.delete(workspaceId);
			}
			const stoppedRemoval = stoppedRemovalOf(workspaceId);
			return result.kind === 'discovered'
				? { kind: 'discovered', discovery: result.facts.discovery, stoppedRemoval }
				: { ...result, stoppedRemoval };
		},
		run,
	};
}
