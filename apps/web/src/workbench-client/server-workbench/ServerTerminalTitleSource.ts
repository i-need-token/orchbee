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
import type { ServerPresentationTitleRow } from './mergeServerTerminalTitle';
import { serverTerminalBindingKey } from './server-terminal-binding';

type PanelAgentOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.panel-agent.result' }
>['outcome'];
type PresentationTitleRow = Extract<
	PanelAgentOutcome,
	{ readonly kind: 'presentation-titles' }
>['presentations'][number];
type PresentationId = PresentationTitleRow['presentationId'];

/** One Presentation's published title, with the Workbench binding the Server read it for. */
export interface ServerPresentationTitle extends ServerPresentationTitleRow {
	readonly presentationId: PresentationId;
}

/** The most Presentations the Server answers in one `presentation-titles` request. */
const MAXIMUM_TITLE_ROWS = 64;
/** A stream the Server closed for capacity is opened again after this long. */
const CAPACITY_RETRY_MILLISECONDS = 5000;

interface PresentationGroup {
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
}

type TitleReading =
	| { readonly kind: 'failed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'titles'; readonly rows: readonly PresentationTitleRow[] };

interface TitleStream {
	readonly key: string;
	readonly presentationIds: readonly PresentationId[];
	requestId: string;
	retry: ReturnType<typeof setTimeout> | null;
	readonly sessionId: WorkbenchSnapshot['sessionId'];
	subscription: ServerSubscription | null;
}

/**
 * The titles the Server publishes for every bound Presentation in this session, in every Workspace,
 * mounted or not. A State stream answers at most 64 Presentations, so they are split across as many
 * streams as they need, and a change of membership reopens only the streams whose Presentations
 * changed. Rows are handed over as they arrive; this source keeps none, because a title that cannot
 * be read keeps whatever its Presentation last showed, so a failed or closed stream changes nothing.
 */
export class ServerTerminalTitleSource {
	readonly #connection: Pick<ServerConnection, 'subscribe'>;
	readonly #receive: (rows: readonly ServerPresentationTitle[]) => void;
	#ended = false;
	#stopEnd: (() => void) | null = null;
	#stopSession: (() => void) | null = null;
	#streams: TitleStream[] = [];

	constructor(
		connection: Pick<ServerConnection, 'subscribe'>,
		session: Pick<ServerWorkbenchSession, 'onEnded' | 'snapshot' | 'subscribe'>,
		receive: (rows: readonly ServerPresentationTitle[]) => void,
	) {
		this.#connection = connection;
		this.#receive = receive;
		// A session that has already ended reports it here, which ends this source at once.
		this.#stopEnd = session.onEnded(() => this.dispose());
		if (this.#ended) return;
		this.#stopSession = session.subscribe((snapshot) => this.#follow(snapshot));
		this.#follow(session.snapshot);
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
		const groups = groupBoundPresentations(snapshot);
		if (
			groups.length === this.#streams.length &&
			groups.every((group, index) => this.#streams[index]?.key === group.key)
		) {
			return;
		}
		const previous = new Map(this.#streams.map((stream) => [stream.key, stream]));
		const opened: TitleStream[] = [];
		this.#streams = groups.map((group) => {
			const kept = previous.get(group.key);
			if (kept !== undefined) {
				previous.delete(group.key);
				return kept;
			}
			const stream: TitleStream = {
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

	#open(stream: TitleStream): void {
		const requestId = createWorkbenchRequestId();
		stream.requestId = requestId;
		try {
			stream.subscription = this.#connection.subscribe(
				'state',
				encodeWorkbenchRequest({
					action: { kind: 'presentation-titles', presentationIds: stream.presentationIds },
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

	#frame(stream: TitleStream, record: Uint8Array): void {
		if (!this.#streams.includes(stream)) return;
		const reading = readTitles(record, stream);
		switch (reading.kind) {
			case 'invalid':
				// A Server of this cohort never sends it, so nothing it reports can be trusted.
				this.dispose();
				return;
			case 'failed':
				// Until the Server publishes titles, and while it cannot, every title stays as it was.
				return;
			case 'titles':
				this.#receive(
					reading.rows.map(({ binding, presentationId, title }) => ({
						binding: binding === null ? null : serverTerminalBindingKey(binding),
						presentationId,
						title,
					})),
				);
				return;
		}
	}

	#closed(stream: TitleStream, retry: boolean): void {
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

function closeStream(stream: TitleStream): void {
	stream.subscription?.close();
	stream.subscription = null;
	if (stream.retry !== null) clearTimeout(stream.retry);
	stream.retry = null;
}

/** Only a bound Presentation has a title to publish, so only those are asked for. */
function groupBoundPresentations(snapshot: WorkbenchSnapshot): PresentationGroup[] {
	const presentationIds: PresentationId[] = [];
	const collect = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
				if (presentation.binding !== null) presentationIds.push(presentation.presentationId);
			}
			return;
		}
		for (const slot of node.children) collect(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) collect(tab.root);
	}
	const groups: PresentationGroup[] = [];
	for (let start = 0; start < presentationIds.length; start += MAXIMUM_TITLE_ROWS) {
		const group = presentationIds.slice(start, start + MAXIMUM_TITLE_ROWS);
		groups.push({ key: [snapshot.sessionId, ...group].join('\0'), presentationIds: group });
	}
	return groups;
}

function readTitles(record: Uint8Array, stream: TitleStream): TitleReading {
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
		outcome.kind !== 'presentation-titles' ||
		outcome.presentations.length !== stream.presentationIds.length ||
		outcome.presentations.some((row, index) => row.presentationId !== stream.presentationIds[index])
	) {
		return { kind: 'invalid' };
	}
	return { kind: 'titles', rows: outcome.presentations };
}
