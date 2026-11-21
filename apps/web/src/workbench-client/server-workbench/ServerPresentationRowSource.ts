import {
	WORKBENCH_PROTOCOL_VERSION,
	type TerminalPresentationId,
	type WorkbenchRequest,
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
import { listBoundPresentationIds } from './listBoundPresentationIds';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

export type ServerPanelAgentAction = Extract<
	WorkbenchRequest,
	{ readonly kind: 'workbench.panel-agent' }
>['action'];
export type ServerPanelAgentOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.panel-agent.result' }
>['outcome'];
type PresentationId = TerminalPresentationId;

/** One Panel Agent read whose answer is a row per requested Presentation. */
export interface ServerPresentationRowRead<Row> {
	readonly action: (presentationIds: readonly PresentationId[]) => ServerPanelAgentAction;
	/** The most Presentations one request may name. */
	readonly limit: number;
	/** The rows of the read's own outcome, or null for any other outcome. */
	readonly rows: (outcome: ServerPanelAgentOutcome) => readonly Row[] | null;
}

/** A stream the Server closed for capacity is opened again after this long. */
const CAPACITY_RETRY_MILLISECONDS = 5000;

interface PresentationGroup {
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
}

type RowReading<Row> =
	| { readonly kind: 'failed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'rows'; readonly rows: readonly Row[] };

interface RowStream {
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
	requestId: string;
	retry: ReturnType<typeof setTimeout> | null;
	readonly sessionId: WorkbenchSnapshot['sessionId'];
	subscription: ServerSubscription | null;
}

/**
 * The rows one read publishes for every bound Presentation in this session, in every Workspace,
 * mounted or not. A State stream answers at most the read's limit of Presentations, so they are
 * split across as many streams as they need, and a change of membership reopens only the streams
 * whose Presentations changed. Rows are handed over as they arrive; this source keeps none, so a
 * failed or closed stream changes nothing its consumer last read.
 */
export class ServerPresentationRowSource<Row extends { readonly presentationId: string }> {
	readonly #connection: Pick<ServerConnection, 'subscribe'>;
	readonly #read: ServerPresentationRowRead<Row>;
	readonly #receive: (rows: readonly Row[]) => void;
	#ended = false;
	#stopEnd: (() => void) | null = null;
	#stopSession: (() => void) | null = null;
	#streams: RowStream[] = [];

	constructor(
		read: ServerPresentationRowRead<Row>,
		connection: Pick<ServerConnection, 'subscribe'>,
		session: Pick<ServerWorkbenchSession, 'onEnded' | 'snapshot' | 'subscribe'>,
		receive: (rows: readonly Row[]) => void,
	) {
		this.#connection = connection;
		this.#read = read;
		this.#receive = receive;
		// A session that has already ended reports it here, which ends this source at once.
		this.#stopEnd = session.onEnded(() => this.dispose());
		if (this.#ended) return;
		this.#stopSession = session.subscribe((snapshot) => this.#follow(snapshot));
		this.#follow(session.snapshot);
	}

	/** Opens again every stream the Server closed, as it does after replacing a lost Worker. */
	reopenClosed(): void {
		if (this.#ended) return;
		for (const stream of this.#streams) {
			if (stream.subscription !== null) continue;
			if (stream.retry !== null) clearTimeout(stream.retry);
			stream.retry = null;
			this.#open(stream);
		}
	}

	/** Closes every stream; nothing arrives afterwards. */
	dispose(): void {
		if (this.#ended) return;
		this.#ended = true;
		this.#stopEnd?.();
		this.#stopSession?.();
		for (const stream of this.#streams) closeStream(stream);
		this.#streams = [];
	}

	#follow(snapshot: WorkbenchSnapshot): void {
		if (this.#ended) return;
		const groups = groupBoundPresentations(snapshot, this.#read.limit);
		if (
			groups.length === this.#streams.length &&
			groups.every((group, index) => this.#streams[index]?.key === group.key)
		) {
			return;
		}
		const previous = new Map(this.#streams.map((stream) => [stream.key, stream]));
		const opened: RowStream[] = [];
		this.#streams = groups.map((group) => {
			const kept = previous.get(group.key);
			if (kept !== undefined) {
				previous.delete(group.key);
				return kept;
			}
			const stream: RowStream = {
				key: group.key,
				presentationIds: group.presentationIds,
				requestId: '',
				retry: null,
				sessionId: snapshot.sessionId,
				subscription: null,
			};
			opened.push(stream);
			return stream;
		});
		for (const stream of previous.values()) closeStream(stream);
		for (const stream of opened) this.#open(stream);
	}

	#open(stream: RowStream): void {
		const requestId = createWorkbenchRequestId();
		stream.requestId = requestId;
		try {
			stream.subscription = this.#connection.subscribe(
				'state',
				encodeWorkbenchRequest({
					action: this.#read.action(stream.presentationIds),
					kind: 'workbench.panel-agent',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: stream.sessionId,
				}),
				{
					onClosed: (reason) => this.#closed(stream, reason === 'capacity'),
					onFrame: (_kind, record) => this.#frame(stream, record),
					// Every frame holds the complete value, so a gap loses nothing.
					onGap: () => undefined,
				},
			);
		} catch {
			// Only an ending connection refuses a stream, and the session ends with it.
			stream.subscription = null;
		}
	}

	#frame(stream: RowStream, record: Uint8Array): void {
		if (!this.#streams.includes(stream)) return;
		const reading = readRows(record, stream, this.#read);
		switch (reading.kind) {
			case 'invalid':
				// A Server of this cohort never sends it, so nothing it reports can be trusted.
				this.dispose();
				return;
			case 'failed':
				// Until the Server publishes rows, and while it cannot, every row stays as it was.
				return;
			case 'rows':
				this.#receive(reading.rows);
				return;
		}
	}

	#closed(stream: RowStream, retry: boolean): void {
		if (!this.#streams.includes(stream)) return;
		stream.subscription = null;
		// Every other reason holds until these Presentations change and their stream is reopened.
		if (!retry) return;
		stream.retry = setTimeout(() => {
			stream.retry = null;
			if (this.#streams.includes(stream) && stream.subscription === null) this.#open(stream);
		}, CAPACITY_RETRY_MILLISECONDS);
	}
}

function closeStream(stream: RowStream): void {
	stream.subscription?.close();
	stream.subscription = null;
	if (stream.retry !== null) clearTimeout(stream.retry);
	stream.retry = null;
}

/** Only a bound Presentation has a terminal to read, so only those are asked for. */
function groupBoundPresentations(snapshot: WorkbenchSnapshot, limit: number): PresentationGroup[] {
	const presentationIds = listBoundPresentationIds(snapshot);
	const groups: PresentationGroup[] = [];
	for (let start = 0; start < presentationIds.length; start += limit) {
		const group = presentationIds.slice(start, start + limit);
		groups.push({ key: [snapshot.sessionId, ...group].join('\0'), presentationIds: group });
	}
	return groups;
}

function readRows<Row extends { readonly presentationId: string }>(
	record: Uint8Array,
	stream: RowStream,
	read: ServerPresentationRowRead<Row>,
): RowReading<Row> {
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
	const rows = read.rows(outcome);
	// The Server answers exactly the requested Presentations, in the order they were named.
	if (
		rows === null ||
		rows.length !== stream.presentationIds.length ||
		rows.some((row, index) => row.presentationId !== stream.presentationIds[index])
	) {
		return { kind: 'invalid' };
	}
	return { kind: 'rows', rows };
}
