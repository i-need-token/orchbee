import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchPanelNode,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import {
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type PanelAgentOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.panel-agent.result' }
>['outcome'];
type TerminalAgentStateRow = Extract<
	PanelAgentOutcome,
	{ readonly kind: 'states' }
>['presentations'][number];
type PresentationId = TerminalAgentStateRow['presentationId'];

/** The most Presentations the Server answers in one `states` request. */
const MAXIMUM_STATE_ROWS = 64;
/**
 * How long a stream that failed keeps showing the Agents it last reported. The Server fails a stream
 * naming a Presentation that was just removed until this Client reopens it, and fails every stream
 * while its Worker restarts; neither moment should blank the Agents that are still there.
 */
const FAILURE_GRACE_MILLISECONDS = 10_000;
/** A stream the Server closed for capacity is opened again after this long. */
const CAPACITY_RETRY_MILLISECONDS = 5000;

const UNOBSERVED_SNAPSHOT: WorkbenchAgentSnapshot = Object.freeze({
	complete: false,
	presentations: new Map<string, WorkbenchAgentObservation>(),
});

interface PresentationGroup {
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
}

type StateReading =
	| { readonly kind: 'failed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'states'; readonly rows: readonly TerminalAgentStateRow[] };

interface StateStream {
	/** Whether this stream has answered at least once. */
	delivered: boolean;
	expiry: ReturnType<typeof setTimeout> | null;
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
	requestId: string;
	retry: ReturnType<typeof setTimeout> | null;
	rows: ReadonlyMap<string, WorkbenchAgentObservation>;
	readonly sessionId: WorkbenchSnapshot['sessionId'];
	subscription: ServerSubscription | null;
}

/**
 * The Agents the Server observes in this session's terminals, read as the TUI reads them: process
 * and screen detection with Agent reports, per exact terminal incarnation. A State stream answers at
 * most 64 Presentations, so the session's Presentations are split across as many streams as they
 * need, and a change of membership reopens only the streams whose Presentations changed.
 */
export class ServerTerminalAgentSource implements WorkbenchAgentSource {
	readonly #connection: Pick<ServerConnection, 'subscribe'>;
	readonly #listeners = new Set<() => void>();
	#complete = false;
	#ended = false;
	#snapshot = UNOBSERVED_SNAPSHOT;
	#stopEnd: (() => void) | null = null;
	#stopSession: (() => void) | null = null;
	#streams: StateStream[] = [];

	constructor(
		connection: Pick<ServerConnection, 'subscribe'>,
		session: Pick<ServerWorkbenchSession, 'onEnded' | 'snapshot' | 'subscribe'>,
	) {
		this.#connection = connection;
		// A session that has already ended reports it here, which ends this source at once.
		this.#stopEnd = session.onEnded(() => this.dispose());
		if (this.#ended) return;
		this.#stopSession = session.subscribe((snapshot) => this.#follow(snapshot));
		this.#follow(session.snapshot);
	}

	getSnapshot(): WorkbenchAgentSnapshot {
		return this.#snapshot;
	}

	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	/** Closes every stream. Nothing is observed afterwards, so no Agent stays shown. */
	dispose(): void {
		if (this.#ended) return;
		this.#ended = true;
		this.#stopEnd?.();
		this.#stopSession?.();
		for (const stream of this.#streams) closeStream(stream);
		this.#streams = [];
		if (this.#snapshot === UNOBSERVED_SNAPSHOT) return;
		this.#snapshot = UNOBSERVED_SNAPSHOT;
		this.#notify();
	}

	#follow(snapshot: WorkbenchSnapshot): void {
		if (this.#ended) return;
		const groups = groupPresentations(snapshot);
		if (
			groups.length === this.#streams.length &&
			groups.every((group, index) => this.#streams[index]?.key === group.key)
		) {
			return;
		}
		const previous = new Map(this.#streams.map((stream) => [stream.key, stream]));
		const known = new Map<string, WorkbenchAgentObservation>();
		for (const stream of this.#streams) {
			for (const [presentationId, observation] of stream.rows) {
				known.set(presentationId, observation);
			}
		}
		const opened: StateStream[] = [];
		this.#streams = groups.map((group) => {
			const kept = previous.get(group.key);
			if (kept !== undefined) {
				previous.delete(group.key);
				return kept;
			}
			// A Presentation that stays keeps its Agent while the stream that now answers it opens.
			const rows = new Map<string, WorkbenchAgentObservation>();
			for (const presentationId of group.presentationIds) {
				const observation = known.get(presentationId);
				if (observation !== undefined) rows.set(presentationId, observation);
			}
			const stream: StateStream = {
				delivered: false,
				expiry: null,
				key: group.key,
				presentationIds: group.presentationIds,
				requestId: '',
				retry: null,
				rows,
				sessionId: snapshot.sessionId,
				subscription: null,
			};
			opened.push(stream);
			return stream;
		});
		for (const stream of previous.values()) closeStream(stream);
		for (const stream of opened) {
			this.#open(stream);
			this.#expireLater(stream);
		}
		this.#publish();
	}

	#open(stream: StateStream): void {
		const requestId = createWorkbenchRequestId();
		stream.requestId = requestId;
		try {
			stream.subscription = this.#connection.subscribe(
				'state',
				encodeWorkbenchRequest({
					action: { kind: 'states', presentationIds: stream.presentationIds },
					kind: 'workbench.panel-agent',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: stream.sessionId,
				}),
				{
					onClosed: (reason) => this.#closed(stream, reason === 'capacity'),
					onFrame: (_kind, record) => this.#receive(stream, record),
					// Every frame holds the complete value, so a gap loses nothing.
					onGap: () => undefined,
				},
			);
		} catch {
			// Only an ending connection refuses a stream, and the session ends with it.
			stream.subscription = null;
		}
	}

	#receive(stream: StateStream, record: Uint8Array): void {
		if (!this.#streams.includes(stream)) return;
		const reading = readStates(record, stream);
		switch (reading.kind) {
			case 'invalid':
				// A Server of this cohort never sends it, so nothing it reports can be trusted.
				this.dispose();
				return;
			case 'failed':
				this.#expireLater(stream);
				return;
			case 'states':
				stream.delivered = true;
				if (stream.expiry !== null) clearTimeout(stream.expiry);
				stream.expiry = null;
				stream.rows = observeRows(reading.rows, stream.rows);
				this.#publish();
				return;
		}
	}

	#closed(stream: StateStream, retry: boolean): void {
		if (!this.#streams.includes(stream)) return;
		stream.subscription = null;
		this.#expireLater(stream);
		// Every other reason holds until these Presentations change and their stream is reopened.
		if (!retry) return;
		stream.retry = setTimeout(() => {
			stream.retry = null;
			if (this.#streams.includes(stream) && stream.subscription === null) this.#open(stream);
		}, CAPACITY_RETRY_MILLISECONDS);
	}

	/** Forgets what a stream last reported once it has failed to answer for the grace period. */
	#expireLater(stream: StateStream): void {
		if (stream.expiry !== null || stream.rows.size === 0) return;
		stream.expiry = setTimeout(() => {
			stream.expiry = null;
			stream.rows = new Map();
			this.#publish();
		}, FAILURE_GRACE_MILLISECONDS);
	}

	#publish(): void {
		// The first moment every Presentation has been answered is the baseline attention starts from.
		if (!this.#complete && this.#streams.every((stream) => stream.delivered)) {
			this.#complete = true;
		}
		const presentations = new Map<string, WorkbenchAgentObservation>();
		for (const stream of this.#streams) {
			for (const [presentationId, observation] of stream.rows) {
				presentations.set(presentationId, observation);
			}
		}
		const current = this.#snapshot;
		if (
			current.complete === this.#complete &&
			current.presentations.size === presentations.size &&
			[...presentations].every(
				([presentationId, observation]) =>
					current.presentations.get(presentationId) === observation,
			)
		) {
			return;
		}
		this.#snapshot = Object.freeze({ complete: this.#complete, presentations });
		this.#notify();
	}

	#notify(): void {
		for (const listener of this.#listeners) listener();
	}
}

function closeStream(stream: StateStream): void {
	stream.subscription?.close();
	stream.subscription = null;
	if (stream.expiry !== null) clearTimeout(stream.expiry);
	if (stream.retry !== null) clearTimeout(stream.retry);
	stream.expiry = null;
	stream.retry = null;
}

function groupPresentations(snapshot: WorkbenchSnapshot): PresentationGroup[] {
	const presentationIds: PresentationId[] = [];
	const collect = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			presentationIds.push(node.primaryPresentation.presentationId);
			for (const child of node.childPresentations) presentationIds.push(child.presentationId);
			return;
		}
		for (const slot of node.children) collect(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) collect(tab.root);
	}
	const groups: PresentationGroup[] = [];
	for (let start = 0; start < presentationIds.length; start += MAXIMUM_STATE_ROWS) {
		const group = presentationIds.slice(start, start + MAXIMUM_STATE_ROWS);
		groups.push({ key: [snapshot.sessionId, ...group].join('\0'), presentationIds: group });
	}
	return groups;
}

function readStates(record: Uint8Array, stream: StateStream): StateReading {
	let response: WorkbenchResponse;
	try {
		response = decodeWorkbenchResponse(record);
	} catch {
		return { kind: 'invalid' };
	}
	if (response.kind !== 'workbench.panel-agent.result' || response.requestId !== stream.requestId) {
		return { kind: 'invalid' };
	}
	const { outcome } = response;
	if (outcome.kind === 'failed') return { kind: 'failed' };
	// The Server answers exactly the requested Presentations, in the order they were named.
	if (
		outcome.kind !== 'states' ||
		outcome.presentations.length !== stream.presentationIds.length ||
		outcome.presentations.some((row, index) => row.presentationId !== stream.presentationIds[index])
	) {
		return { kind: 'invalid' };
	}
	return { kind: 'states', rows: outcome.presentations };
}

/** Keeps each unchanged observation's identity, so a frame that changes nothing changes nothing. */
function observeRows(
	rows: readonly TerminalAgentStateRow[],
	previous: ReadonlyMap<string, WorkbenchAgentObservation>,
): ReadonlyMap<string, WorkbenchAgentObservation> {
	const observed = new Map<string, WorkbenchAgentObservation>();
	for (const { presentationId, snapshot } of rows) {
		// A terminal without a named Agent has none to show.
		if (snapshot === null || snapshot.agent === null) continue;
		const { generation, hostEpoch, hostTerminalId } = snapshot.binding;
		const binding = `${generation}:${hostEpoch}:${hostTerminalId}`;
		const sequence = BigInt(snapshot.sequence);
		const prior = previous.get(presentationId);
		observed.set(
			presentationId,
			prior?.agent === snapshot.agent &&
				prior.binding === binding &&
				prior.sequence === sequence &&
				prior.state === snapshot.state
				? prior
				: Object.freeze({ agent: snapshot.agent, binding, sequence, state: snapshot.state }),
		);
	}
	return observed;
}
