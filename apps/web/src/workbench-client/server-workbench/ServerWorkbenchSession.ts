import {
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	type WorkbenchCommand,
	type WorkbenchCommandResult,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import {
	createClientServerRequestId,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	ServerStreamRefusedError,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';

export type ServerWorkbenchAction = WorkbenchCommand['action'];
export type ServerWorkbenchOutcome = WorkbenchCommandResult['outcome'];

export type ServerWorkbenchSubmission =
	/** The Server answered. Any commit it reports has already reached this session's snapshot. */
	| { readonly kind: 'settled'; readonly outcome: ServerWorkbenchOutcome }
	/** The Server refused the stream before running the request. */
	| { readonly kind: 'not-submitted' }
	/** The request may have taken effect. It is never replayed. */
	| { readonly kind: 'outcome-unknown' };

export type ServerWorkbenchEnd =
	| { readonly kind: 'connection-ended' }
	| { readonly kind: 'protocol-violation' }
	| { readonly kind: 'publication-ended' };

export type ServerWorkbenchConnection = Pick<
	ServerConnection,
	'exchange' | 'onEnded' | 'subscribe'
>;

type SnapshotListener = (snapshot: WorkbenchSnapshot) => void;

interface PublicationWaiter {
	readonly resolve: () => void;
	readonly revision: bigint;
}

/**
 * The Workbench authority one Client connection observes. The Server's State stream is the only
 * source of its snapshot: a command never edits it, so every Client converges on what was
 * published. Commands run one at a time because each one names the revision it was decided from.
 */
export class ServerWorkbenchSession {
	readonly #connection: ServerWorkbenchConnection;
	readonly #endListeners = new Set<(end: ServerWorkbenchEnd) => void>();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #stopConnectionWatch: () => void;
	#end: ServerWorkbenchEnd | null = null;
	#queue: Promise<unknown> = Promise.resolve();
	#snapshot: WorkbenchSnapshot;
	#subscription: ServerSubscription | null = null;
	#waiters: PublicationWaiter[] = [];

	private constructor(connection: ServerWorkbenchConnection, snapshot: WorkbenchSnapshot) {
		this.#connection = connection;
		this.#snapshot = snapshot;
		this.#stopConnectionWatch = connection.onEnded(() =>
			this.#finish({ kind: 'connection-ended' }),
		);
	}

	/** Resolves with the first published snapshot, and rejects when none can be read. */
	static open(connection: ServerWorkbenchConnection): Promise<ServerWorkbenchSession> {
		return new Promise((resolve, reject) => {
			let session: ServerWorkbenchSession | null = null;
			let stopWaiting = (): void => undefined;
			const fail = (end: ServerWorkbenchEnd): void => {
				if (session === null) {
					subscription.close();
					stopWaiting();
					reject(new Error(`The Workbench is unavailable: ${end.kind}.`));
				} else {
					session.#finish(end);
				}
			};
			// An ended connection refuses this call, which rejects the promise.
			const subscription = connection.subscribe(
				'state',
				encodeWorkbenchRequest({
					kind: 'workbench.snapshot.read',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId: createWorkbenchRequestId(),
				}),
				{
					onClosed: () => fail({ kind: 'publication-ended' }),
					onFrame: (_kind, record) => {
						const snapshot = readPublishedSnapshot(record);
						if (snapshot === null) {
							fail({ kind: 'protocol-violation' });
						} else if (session === null) {
							stopWaiting();
							session = new ServerWorkbenchSession(connection, snapshot);
							session.#subscription = subscription;
							resolve(session);
						} else {
							session.#publish(snapshot);
						}
					},
					// The next frame is a complete snapshot, so a gap in this stream loses nothing.
					onGap: () => undefined,
				},
			);
			// A connection that ended while the stream opened reports it here.
			stopWaiting = connection.onEnded(() => fail({ kind: 'connection-ended' }));
		});
	}

	get ended(): ServerWorkbenchEnd | null {
		return this.#end;
	}

	get snapshot(): WorkbenchSnapshot {
		return this.#snapshot;
	}

	subscribe(listener: SnapshotListener): () => void {
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	}

	onEnded(listener: (end: ServerWorkbenchEnd) => void): () => void {
		if (this.#end !== null) {
			listener(this.#end);
			return () => undefined;
		}
		this.#endListeners.add(listener);
		return () => this.#endListeners.delete(listener);
	}

	submit(action: ServerWorkbenchAction): Promise<ServerWorkbenchSubmission> {
		const submission = this.#queue.then(() => this.#submitNext(action));
		this.#queue = submission;
		return submission;
	}

	dispose(): void {
		this.#finish({ kind: 'connection-ended' });
	}

	async #submitNext(action: ServerWorkbenchAction): Promise<ServerWorkbenchSubmission> {
		if (this.#end !== null) return { kind: 'not-submitted' };
		const requestId = createWorkbenchRequestId();
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(
				encodeWorkbenchRequest({
					action,
					expectedRevision: this.#snapshot.revision,
					kind: 'workbench.command',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: this.#snapshot.sessionId,
				}),
			);
		} catch (error) {
			return error instanceof ServerStreamRefusedError
				? { kind: 'not-submitted' }
				: { kind: 'outcome-unknown' };
		}
		const outcome = this.#readOutcome(record, requestId);
		if (outcome === null) return { kind: 'outcome-unknown' };
		const revision = committedRevision(outcome);
		if (revision !== null) await this.#published(revision);
		return { kind: 'settled', outcome };
	}

	#readOutcome(record: Uint8Array, requestId: string): ServerWorkbenchOutcome | null {
		try {
			const response = decodeWorkbenchResponse(record);
			if (response.kind === 'workbench.command.result' && response.requestId === requestId) {
				return response.outcome;
			}
		} catch {
			// The response is a defect of this cohort, reported below.
		}
		this.#finish({ kind: 'protocol-violation' });
		return null;
	}

	#published(revision: bigint): Promise<void> {
		if (this.#end !== null || BigInt(this.#snapshot.revision) >= revision) {
			return Promise.resolve();
		}
		return new Promise((resolve) => this.#waiters.push({ resolve, revision }));
	}

	#publish(snapshot: WorkbenchSnapshot): void {
		this.#snapshot = snapshot;
		for (const listener of this.#snapshotListeners) listener(snapshot);
		const revision = BigInt(snapshot.revision);
		const waiting: PublicationWaiter[] = [];
		for (const waiter of this.#waiters) {
			if (revision >= waiter.revision) waiter.resolve();
			else waiting.push(waiter);
		}
		this.#waiters = waiting;
	}

	#finish(end: ServerWorkbenchEnd): void {
		if (this.#end !== null) return;
		this.#end = end;
		this.#stopConnectionWatch();
		this.#subscription?.close();
		this.#subscription = null;
		for (const waiter of this.#waiters) waiter.resolve();
		this.#waiters = [];
		for (const listener of this.#endListeners) listener(end);
		this.#endListeners.clear();
		this.#snapshotListeners.clear();
	}
}

function createWorkbenchRequestId() {
	return WorkbenchRequestIdSchema.parse(createClientServerRequestId());
}

function readPublishedSnapshot(record: Uint8Array): WorkbenchSnapshot | null {
	try {
		const response = decodeWorkbenchResponse(record);
		return response.kind === 'workbench.snapshot.result' && response.outcome.kind === 'available'
			? response.outcome.snapshot
			: null;
	} catch {
		return null;
	}
}

/** The revision a publication must reach before the commit is visible; null if nothing committed. */
function committedRevision(outcome: ServerWorkbenchOutcome): bigint | null {
	switch (outcome.kind) {
		case 'rejected':
		case 'denied':
		case 'unavailable':
		case 'unchanged':
		case 'launch-profile-unavailable':
		case 'launch-profile-ambiguous':
			return null;
		case 'tab-close-accepted':
		case 'panel-close-accepted':
		case 'workspace-close-accepted':
			return BigInt(outcome.acceptedRevision);
		default:
			return BigInt(outcome.revision);
	}
}
