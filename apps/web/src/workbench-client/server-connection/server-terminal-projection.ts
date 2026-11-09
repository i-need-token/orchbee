/**
 * Opens one Server terminal as a renderer projection. The viewer and controller share a lane, so
 * closing the lane releases both. Poll is the only way to take a record, and the Server's hint says
 * when one is waiting. This holds no terminal semantics; the Host and Worker own those.
 */
import {
	CanonicalUint64TextSchema,
	HistoryChangedSchema,
	MessageKind,
	PayloadEncoding,
	SelectionChangedSchema,
	type TerminalGeometry,
	TerminalExitedSchema,
	TerminalFailedSchema,
	decodeControlJson,
	decodeProtocolEnvelope,
	encodeInputRequest,
} from '@orchbee/terminal-protocol';
import {
	type ProjectedAttachmentBootstrap,
	ProjectedAttachmentBootstrapSchema,
	type ProjectedTerminalCommand,
	type ProjectedTerminalEvent,
	type ProjectedTerminalFailure,
} from '@orchbee/workbench-client-protocol';

import type {
	ProjectedTerminalTakeControlOutcome,
	ProjectedTerminalTransport,
	TerminalProjection,
} from '../terminal-projection';
import {
	type ClientServerControlAction,
	type ClientServerControlOutcome,
	type ClientServerRequest,
	type ClientServerResponse,
	type ClientServerViewerAction,
	type ClientServerViewerOutcome,
	MAXIMUM_PASTE_CONTENT_BYTES,
	MAXIMUM_SEMANTIC_INPUT_BYTES,
	bytesToLowercaseHex,
	createClientServerRequestId,
	decodeClientServerResponse,
	encodeClientServerPaste,
	encodeClientServerRequest,
} from './client-server-carrier';
import { type ServerConnection, ServerConnectionEndedError } from './server-connection';
import { ServerSelectionCopy, readSelectionAnswer } from './server-selection-copy';
import {
	type ServerHostColors,
	ServerHostColorsReport,
	type ServerHostColorsSource,
} from './ServerHostColorsReport';
import { ServerTerminalEffects } from './ServerTerminalEffects';

export interface ServerTerminalBinding {
	readonly sessionId: string;
	readonly terminalId: string;
	readonly workerId: string;
}

/** A refusal while opening a projection, in the renderer's failure vocabulary. */
export class ServerTerminalProjectionError extends Error {
	readonly code: string;
	readonly retryable: boolean;

	constructor(code: string, retryable: boolean) {
		super(`Terminal projection failed: ${code}.`);
		this.name = 'ServerTerminalProjectionError';
		this.code = code;
		this.retryable = retryable;
	}
}

export interface ServerTerminalProjection extends TerminalProjection {
	/** One focus gesture; abort prevents any transfer that has not yet been submitted. */
	activateFocus(initial: boolean, signal: AbortSignal): Promise<void>;
	/** Closes the transport, and settles once what was already typed has gone and the lane closed. */
	close(): Promise<void>;
}

/** How the Host settled the exact terminal a projection attached, as its renderer receives it. */
export type ServerTerminalSettlement = Extract<
	ProjectedTerminalEvent,
	{ readonly kind: 'terminal-exited' | 'terminal-failed' }
>;

/** Pacing of an empty Poll for a viewer that cannot yet rely on a Server hint. */
const UNHINTED_POLL_DELAY_MILLISECONDS = 8;
/** How long a lost hint can hide a record; it also bounds how long a Poll-carried notice waits. */
const FALLBACK_POLL_DELAY_MILLISECONDS = 250;
const BUSY_RETRY_DELAYS_MILLISECONDS = [4, 8, 16, 32, 64, 128, 256] as const;
const MINIMUM_ROW_READ_BYTES = 112;
const MAXIMUM_ROW_READ_BYTES = 8 * 1024 * 1024;
const MAXIMUM_QUEUED_COMMANDS = 1_024;
/** How long closing waits on queued input. The next surface for the terminal waits on this in turn. */
const INPUT_DELIVERY_LIMIT_MILLISECONDS = 500;
/**
 * The bootstrap contract requires a theme, and only a renderer's theme participant reads it. A Server
 * terminal's colors come from its Host, which the Worker configures at launch, so these never paint.
 */
const UNREAD_BOOTSTRAP_THEME = {
	colorScheme: 'dark',
	defaultBackgroundRgb: 0,
	defaultCursorRgb: null,
	defaultForegroundRgb: 0xff_ff_ff,
	palette: Array.from({ length: 256 }, () => 0),
} as const;

const utf8Encoder = new TextEncoder();

const SEMANTIC_INPUT_ROUTES = {
	'focus-input': { inputKind: 'focus', messageKind: MessageKind.focusInput },
	'key-input': { inputKind: 'key', messageKind: MessageKind.keyInput },
	'mouse-input': { inputKind: 'mouse', messageKind: MessageKind.mouseInput },
	'text-input': { inputKind: 'text', messageKind: MessageKind.textInput },
} as const;

type SemanticInputCommand = Extract<
	ProjectedTerminalCommand,
	{ readonly kind: keyof typeof SEMANTIC_INPUT_ROUTES }
>;

/** Runs asynchronous steps one at a time so a retry can never overtake a later step. */
class SerialSteps {
	readonly #queue: (() => Promise<void>)[] = [];
	#onIdle: (() => void) | null = null;
	#running = false;

	get length(): number {
		return this.#queue.length;
	}

	push(step: () => Promise<void>): void {
		this.#queue.push(step);
		if (!this.#running) void this.#run();
	}

	clear(): void {
		this.#queue.length = 0;
	}

	/** Calls back once no step is left to run, which may be at once. */
	whenIdle(listener: () => void): void {
		if (this.#running) this.#onIdle = listener;
		else listener();
	}

	async #run(): Promise<void> {
		this.#running = true;
		for (let step = this.#queue.shift(); step !== undefined; step = this.#queue.shift()) {
			await step();
		}
		this.#running = false;
		this.#onIdle?.();
	}
}

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => {
		setTimeout(resolve, milliseconds);
	});
}

function failureContext(
	command: ProjectedTerminalCommand,
): Pick<ProjectedTerminalFailure, 'operation' | 'requestSequence'> {
	switch (command.kind) {
		case 'apply-terminal-theme':
		case 'reconcile-terminal-theme':
		case 'frame-applied':
		case 'need-snapshot':
		case 'set-projection-demand':
			return { operation: 'protocol' };
		case 'clear-terminal':
		case 'reset-terminal':
			return { operation: 'control' };
		case 'clipboard-write-decision':
			return { operation: 'clipboard', requestSequence: command.payload.sequence };
		case 'selection-copy':
			return { operation: 'copy', requestSequence: command.payload.requestSequence };
		case 'link-open':
			return { operation: 'link', requestSequence: command.payload.requestSequence };
		case 'read-rows':
			return { operation: 'history', requestSequence: command.payload.requestSequence };
		case 'rows-applied':
		case 'history-observed':
		case 'set-history-demand':
			return { operation: 'history' };
		case 'selection-command':
		case 'selection-gesture':
			return { operation: 'selection' };
		case 'resize-terminal':
			return { operation: 'resize' };
		case 'focus-input':
		case 'key-input':
		case 'mouse-input':
		case 'paste-input':
		case 'text-input':
			return { operation: 'input' };
	}
}

class ServerProjectedTerminalTransport implements ProjectedTerminalTransport {
	readonly projectionBufferTrust = 'external' as const;
	readonly #bootstrap: ProjectedAttachmentBootstrap;
	readonly #connection: ServerConnection;
	readonly #controlSteps = new SerialSteps();
	readonly #effects: ServerTerminalEffects;
	readonly #hostColors: ServerHostColorsReport | null;
	readonly #laneId: bigint;
	readonly #onSettled: ((settlement: ServerTerminalSettlement) => void) | undefined;
	readonly #selectionCopy: ServerSelectionCopy;
	readonly #viewerSteps = new SerialSteps();
	readonly #unsubscribeEnded: () => void;
	readonly #unwatchViewerReady: () => void;
	#capabilityRevision = 1n;
	#closed = false;
	#closing: Promise<void> | null = null;
	#subscriberReadyResolve: (() => void) | null = null;
	readonly #subscriberReady = new Promise<void>((resolve) => {
		this.#subscriberReadyResolve = resolve;
	});

	#controllerId: string | null;
	#geometry: TerminalGeometry;
	#observingControl = false;
	#observeDue = 0;
	#latestOutcomeRevision: bigint | null = null;
	#listener: ((event: ProjectedTerminalEvent) => void) | null = null;
	#nextControlSequence = 1n;
	#owedFullFrame = true;
	#paused = false;
	#pollInFlight = false;
	#pollTimer: ReturnType<typeof setTimeout> | null = null;
	#recordReady = false;
	#unavailable: (() => void) | null = null;

	constructor(
		connection: ServerConnection,
		laneId: bigint,
		bootstrap: ProjectedAttachmentBootstrap,
		controllerId: string | null,
		geometry: TerminalGeometry,
		hostColors: ServerHostColorsSource | undefined,
		onSettled: ((settlement: ServerTerminalSettlement) => void) | undefined,
	) {
		this.#bootstrap = bootstrap;
		this.#connection = connection;
		this.#controllerId = controllerId;
		this.#geometry = geometry;
		this.#effects = new ServerTerminalEffects(bootstrap.attachmentId, (event) => this.#post(event));
		this.#hostColors =
			hostColors === undefined
				? null
				: new ServerHostColorsReport({
						enqueue: (step) => this.#enqueueHostColors(step),
						source: hostColors,
						submit: (colors) => this.#submitHostColors(colors),
					});
		this.#laneId = laneId;
		this.#onSettled = onSettled;
		this.#selectionCopy = new ServerSelectionCopy({
			attachmentId: bootstrap.attachmentId,
			exchange: (action) => this.#viewerResponseWithRetry(action),
			onChunkAcknowledged: () => this.#pollAfterProgress(),
			post: (event) => this.#post(event),
		});
		this.#unsubscribeEnded = connection.onEnded(() => this.#retire(true));
		this.#unwatchViewerReady = connection.watchViewerReady(
			BigInt(`0x${bootstrap.attachmentId}`),
			() => {
				this.#recordReady = true;
				this.#pollNow();
			},
		);
		if (controllerId !== null) this.#hostColors?.granted();
	}

	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void {
		this.#listener = listener;
		this.#unavailable = unavailable;
		this.#subscriberReadyResolve?.();
		this.#subscriberReadyResolve = null;
		this.#pollNow();
		return () => {
			this.#listener = null;
			this.#unavailable = null;
		};
	}

	send(command: ProjectedTerminalCommand): void {
		if (this.#closed || this.#closing !== null) return;
		const attachmentId = this.#bootstrap.attachmentId;
		switch (command.kind) {
			case 'frame-applied':
				this.#viewerStep(command, { ...command.payload, kind: 'frame-applied' }, true);
				return;
			case 'need-snapshot':
				this.#owedFullFrame = true;
				this.#viewerStep(command, { ...command.payload, kind: 'need-snapshot' }, true);
				return;
			case 'rows-applied':
				this.#viewerStep(command, { ...command.payload, kind: 'rows-applied' }, false);
				return;
			case 'history-observed':
				this.#viewerStep(command, { ...command.payload, kind: 'history-observed' }, false);
				return;
			case 'set-history-demand':
				this.#viewerStep(command, { ...command.payload, kind: 'set-history-demand' }, false);
				return;
			case 'set-projection-demand':
				this.#paused = command.payload.demand === 'paused';
				this.#viewerStep(command, { ...command.payload, kind: 'set-projection-demand' }, true);
				return;
			case 'read-rows':
				this.#readRows(command);
				return;
			case 'focus-input':
			case 'key-input':
			case 'mouse-input':
			case 'text-input':
				this.#semanticInput(command);
				return;
			case 'paste-input':
				this.#paste(command);
				return;
			case 'resize-terminal': {
				this.#geometry = {
					cellPixelHeight: command.payload.cellPixelHeight,
					cellPixelWidth: command.payload.cellPixelWidth,
					columns: command.payload.columns,
					rows: command.payload.rows,
				};
				const { cellPixelHeight, cellPixelWidth, columns, rows } = command.payload;
				this.#controlAction(command, (controllerId, sequence) => ({
					attachmentId,
					controllerId,
					geometry: { cellPixelHeight, cellPixelWidth, columns, rows },
					kind: 'resize',
					sequence,
				}));
				return;
			}
			case 'clear-terminal':
				this.#screenAction(command, 'clear');
				return;
			case 'reset-terminal':
				this.#screenAction(command, 'reset');
				return;
			case 'clipboard-write-decision':
				if (!this.#effects.decide(command.payload)) {
					this.#fail(
						command,
						'clipboard-decision-mismatch',
						'The clipboard decision does not answer the waiting request.',
						false,
					);
				}
				return;
			case 'selection-gesture':
				this.#selectionChange(command, { ...command.payload, kind: 'selection-gesture' });
				return;
			case 'selection-command':
				this.#selectionChange(command, { ...command.payload, kind: 'selection-command' });
				return;
			case 'selection-copy':
				// Behind any queued gesture, so it copies what the user last selected.
				this.#enqueueViewerStep(command, () =>
					this.#selectionCopy.start(command.payload.requestSequence),
				);
				return;
			default:
				this.#fail(
					command,
					'unsupported-message',
					`The Server client does not handle ${command.kind} yet.`,
					false,
				);
		}
	}

	/**
	 * Nothing more comes from or goes to the surface. What it already accepted still goes, one step at
	 * a time as ever, and the lane holding control closes behind it: a person typing as a layout
	 * changes is typing into a surface about to be replaced.
	 */
	close(): Promise<void> {
		this.#subscriberReadyResolve?.();
		this.#subscriberReadyResolve = null;
		this.#closing ??= new Promise((resolve) => {
			this.#listener = null;
			this.#unavailable = null;
			this.#viewerSteps.clear();
			const finish = (): void => {
				clearTimeout(limit);
				this.#retire(false);
				resolve();
			};
			const limit = setTimeout(finish, INPUT_DELIVERY_LIMIT_MILLISECONDS);
			this.#controlSteps.whenIdle(finish);
		});
		return this.#closing;
	}

	activateFocus(initial: boolean, signal: AbortSignal): Promise<void> {
		this.#observingControl = true;
		return new Promise((resolve) => {
			this.#controlSteps.push(async () => {
				try {
					await this.#subscriberReady;
					if (signal.aborted || this.#closed || this.#closing !== null) return;
					const outcome = this.#controlOutcome(
						await this.#exchange({
							kind: 'terminal.control',
							requestId: createClientServerRequestId(),
							action:
								initial && this.#controllerId !== null
									? { attachmentId: this.#bootstrap.attachmentId, kind: 'observe' }
									: {
											attachmentId: this.#bootstrap.attachmentId,
											kind: 'activate',
											geometry: this.#geometry,
										},
						}),
					);
					if (outcome.kind !== 'observed' || this.#closed || this.#closing !== null) return;
					this.#adoptController(outcome.state);
					this.#observeDue = Date.now() + 250;
					if (signal.aborted || this.#controllerId !== null) return;
					// One mutation at most, never a retry of an uncertain or refused gesture.
					await this.takeControl();
				} catch {
					/* Ownership remains whatever was last confirmed; a new gesture may try again. */
				} finally {
					resolve();
				}
			});
		});
	}

	#adoptController(
		state: Extract<ClientServerControlOutcome, { kind: 'observed' }>['state'],
	): void {
		this.#setController(state.controllerId);
		const next = BigInt(state.sequence) + 1n;
		if (next > this.#nextControlSequence) this.#nextControlSequence = next;
	}

	#observeController(): void {
		if (!this.#observingControl || Date.now() < this.#observeDue) return;
		this.#observeDue = Date.now() + 250;
		this.#controlSteps.push(async () => {
			if (this.#closed || this.#closing !== null) return;
			try {
				const outcome = this.#controlOutcome(
					await this.#exchange({
						kind: 'terminal.control',
						requestId: createClientServerRequestId(),
						action: { kind: 'observe', attachmentId: this.#bootstrap.attachmentId },
					}),
				);
				if (!this.#closed && this.#closing === null && outcome.kind === 'observed')
					this.#adoptController(outcome.state);
			} catch {
				/* A later bounded observation can read the Server again, never transfer control. */
			}
		});
	}

	/**
	 * Nothing here repeats a takeover: a busy Server read nothing and the person may ask again, and
	 * one that may have taken effect is theirs to repeat. An attachment the Server no longer has
	 * retires, and the surface's next attachment views until asked again.
	 */
	async takeControl(): Promise<ProjectedTerminalTakeControlOutcome> {
		if (this.#closed || this.#closing !== null) return 'refused';
		let outcome: ClientServerControlOutcome;
		try {
			outcome = this.#controlOutcome(
				await this.#exchange({
					action: { attachmentId: this.#bootstrap.attachmentId, kind: 'takeover' },
					kind: 'terminal.control',
					requestId: createClientServerRequestId(),
				}),
			);
		} catch {
			return this.#closed ? 'refused' : 'unknown';
		}
		if (this.#closed) return 'refused';
		switch (outcome.kind) {
			case 'acquired':
				this.#setController(outcome.controllerId);
				return 'acquired';
			case 'outcome-unknown':
				return 'unknown';
			case 'rejected':
				if (outcome.reason === 'viewer-unavailable') this.#retire(true);
				return 'refused';
			default:
				return 'refused';
		}
	}

	#retire(notify: boolean): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#subscriberReadyResolve?.();
		this.#subscriberReadyResolve = null;
		this.#unsubscribeEnded();
		this.#unwatchViewerReady();
		this.#hostColors?.dispose();
		if (this.#pollTimer !== null) clearTimeout(this.#pollTimer);
		this.#pollTimer = null;
		this.#viewerSteps.clear();
		this.#controlSteps.clear();
		this.#selectionCopy.retire();
		this.#connection.closeLane(this.#laneId);
		const unavailable = this.#unavailable;
		this.#unavailable = null;
		if (notify) unavailable?.();
	}

	#post(event: ProjectedTerminalEvent): void {
		if (!this.#closed) this.#listener?.(event);
	}

	/** The renderer learns the end first, so an observer that fails cannot hide it from the person. */
	#settle(settlement: ServerTerminalSettlement): void {
		this.#post(settlement);
		this.#onSettled?.(settlement);
	}

	#fail(
		command: ProjectedTerminalCommand,
		code: string,
		message: string,
		retryable: boolean,
	): void {
		this.#post({
			failure: { code, message, ...failureContext(command), retryable },
			kind: 'failure',
		});
	}

	#protocolFailure(code: string, message: string): void {
		this.#post({
			failure: { code, message, operation: 'protocol', retryable: false },
			kind: 'failure',
		});
	}

	#selectionEventFailure(message: string): void {
		this.#post({
			failure: {
				code: 'invalid-selection-event',
				message,
				operation: 'selection',
				retryable: false,
			},
			kind: 'failure',
		});
	}

	/** A connection that ended retires this transport through its own listener. */
	#report(command: ProjectedTerminalCommand, error: unknown): void {
		if (error instanceof ServerConnectionEndedError) return;
		this.#fail(
			command,
			'outcome-unknown',
			error instanceof Error ? error.message : 'The Server exchange failed.',
			false,
		);
	}

	async #exchange(request: ClientServerRequest | Uint8Array): Promise<ClientServerResponse> {
		const frame = request instanceof Uint8Array ? request : encodeClientServerRequest(request);
		return decodeClientServerResponse(
			await this.#connection.exchange(frame, { laneId: this.#laneId }),
		);
	}

	#controlOutcome(response: ClientServerResponse): ClientServerControlOutcome {
		if (response.kind !== 'terminal.control.result') {
			throw new Error('The Server answered a control action with another message.');
		}
		return response.outcome;
	}

	#viewerResponse(action: ClientServerViewerAction): Promise<ClientServerResponse> {
		return this.#exchange({
			action,
			kind: 'terminal.viewer',
			requestId: createClientServerRequestId(),
		});
	}

	#enqueueViewerStep(command: ProjectedTerminalCommand, step: () => Promise<void>): void {
		if (this.#viewerSteps.length >= MAXIMUM_QUEUED_COMMANDS) {
			this.#fail(command, 'renderer-backpressure', 'Too many terminal commands are queued.', true);
			return;
		}
		this.#viewerSteps.push(step);
	}

	#viewerStep(
		command: ProjectedTerminalCommand,
		action: ClientServerViewerAction,
		wakesPoll: boolean,
	): void {
		this.#enqueueViewerStep(command, async () => {
			if (this.#closed) return;
			try {
				const outcome = await this.#viewerWithRetry(action);
				if (outcome.kind === 'rejected' || outcome.kind === 'rows-rejected') {
					this.#fail(command, outcome.reason, `The Server refused: ${outcome.reason}.`, false);
				} else if (outcome.kind === 'unavailable') {
					this.#fail(command, outcome.reason, `Unavailable: ${outcome.reason}.`, true);
				}
			} catch (error) {
				this.#report(command, error);
			}
			if (wakesPoll) this.#pollAfterProgress();
		});
	}

	/** A busy Server took nothing, so each retry is a new request. */
	async #viewerResponseWithRetry(action: ClientServerViewerAction): Promise<ClientServerResponse> {
		for (const wait of BUSY_RETRY_DELAYS_MILLISECONDS) {
			const response = await this.#viewerResponse(action);
			const busy =
				response.kind === 'terminal.viewer.result' &&
				response.outcome.kind === 'unavailable' &&
				response.outcome.reason === 'server-busy';
			if (!busy) return response;
			await delay(wait);
			if (this.#closed) return response;
		}
		return this.#viewerResponse(action);
	}

	async #viewerWithRetry(action: ClientServerViewerAction): Promise<ClientServerViewerOutcome> {
		const response = await this.#viewerResponseWithRetry(action);
		if (response.kind !== 'terminal.viewer.result') {
			throw new Error('The Server answered a viewer action with another message.');
		}
		return response.outcome;
	}

	/**
	 * The renderer keeps one gesture in flight until it hears back, so every path here must answer.
	 * A Host failure keeps its own code: the renderer retires a gesture on `stale-generation` or
	 * `history-evicted` without telling the user.
	 */
	#selectionChange(command: ProjectedTerminalCommand, action: ClientServerViewerAction): void {
		this.#enqueueViewerStep(command, async () => {
			if (this.#closed) return;
			try {
				this.#settleSelectionChange(command, await this.#viewerResponseWithRetry(action));
			} catch (error) {
				this.#report(command, error);
			}
		});
	}

	#settleSelectionChange(command: ProjectedTerminalCommand, response: ClientServerResponse): void {
		const answer = readSelectionAnswer(response);
		if (answer.kind === 'refused') {
			this.#fail(command, answer.code, answer.message, answer.retryable);
			return;
		}
		const changed =
			answer.kind === 'record' && answer.envelope.kind === MessageKind.selectionChanged
				? decodeControlJson(answer.envelope.payload, SelectionChangedSchema)
				: null;
		if (changed?.ok !== true || changed.value.attachmentId !== this.#bootstrap.attachmentId) {
			this.#fail(
				command,
				'invalid-selection-response',
				'The Host answered a selection change with another record.',
				false,
			);
			return;
		}
		this.#post({ event: changed.value, kind: 'selection-changed' });
	}

	#readRows(command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>): void {
		const { payload } = command;
		const shared = {
			attachmentId: this.#bootstrap.attachmentId,
			generation: payload.generation,
			maximumBytes: Math.max(
				MINIMUM_ROW_READ_BYTES,
				Math.min(payload.maximumBytes, MAXIMUM_ROW_READ_BYTES),
			),
			requestSequence: payload.requestSequence,
			revision: payload.revision,
			rowCount: payload.rowCount,
			...(payload.detail === 'render' ? { detail: 'render' as const } : {}),
		};
		let action: ClientServerViewerAction;
		if (payload.origin.kind === 'committed' || payload.origin.kind === 'tail') {
			action = { ...shared, kind: 'read-viewport-rows', origin: payload.origin };
		} else if (payload.origin.kind === 'absolute') {
			// A viewport read, such as the one revealing a find match, pins the window its rows-applied
			// acknowledges; any other purpose reads buffer rows.
			action =
				payload.purpose === 'viewport'
					? { ...shared, kind: 'read-viewport-rows', origin: payload.origin }
					: { ...shared, kind: 'read-rows', startRow: payload.origin.row };
		} else {
			this.#fail(command, 'unsupported-message', 'This row origin is unavailable.', false);
			return;
		}
		this.#viewerSteps.push(async () => {
			if (this.#closed) return;
			try {
				this.#settleRows(
					command,
					await this.#exchange({
						action,
						kind: 'terminal.viewer',
						requestId: createClientServerRequestId(),
					}),
				);
			} catch (error) {
				this.#report(command, error);
			}
		});
	}

	#settleRows(command: ProjectedTerminalCommand, response: ClientServerResponse): void {
		if (response.kind === 'terminal.viewer.record') {
			const envelope = decodeProtocolEnvelope(response.hostRecord);
			if (envelope.ok && envelope.value.payloadEncoding === PayloadEncoding.rowWindow) {
				this.#post({ kind: 'rows', rows: envelope.value.payload.slice() });
				return;
			}
		} else if (response.kind === 'terminal.viewer.result' && 'reason' in response.outcome) {
			const { reason } = response.outcome;
			this.#fail(command, reason, `Terminal rows are unavailable: ${reason}.`, false);
			return;
		}
		this.#fail(command, 'invalid-row-window', 'The Server returned no row window.', false);
	}

	#semanticInput(command: SemanticInputCommand): void {
		const route = SEMANTIC_INPUT_ROUTES[command.kind];
		const encoded = encodeInputRequest(route.messageKind, command.payload);
		if (!encoded.ok) {
			this.#fail(command, encoded.code, encoded.detail, false);
			return;
		}
		if (encoded.value.byteLength > MAXIMUM_SEMANTIC_INPUT_BYTES) {
			this.#fail(command, 'limit-exceeded', 'The input record exceeds its limit.', false);
			return;
		}
		const payload = bytesToLowercaseHex(encoded.value);
		this.#controlAction(command, (controllerId, sequence) => ({
			attachmentId: this.#bootstrap.attachmentId,
			controllerId,
			inputKind: route.inputKind,
			kind: 'semantic-input',
			payload,
			sequence,
		}));
	}

	#paste(command: Extract<ProjectedTerminalCommand, { readonly kind: 'paste-input' }>): void {
		const content = utf8Encoder.encode(command.payload.text);
		if (content.byteLength === 0 || content.byteLength > MAXIMUM_PASTE_CONTENT_BYTES) {
			this.#fail(command, 'limit-exceeded', 'The pasted text exceeds its limit.', false);
			return;
		}
		this.#controlStep(command, (controllerId, sequence) =>
			encodeClientServerPaste({
				attachmentId: this.#bootstrap.attachmentId,
				content,
				controllerId,
				requestId: createClientServerRequestId(),
				sequence,
			}),
		);
	}

	#controlAction(
		command: ProjectedTerminalCommand,
		action: (controllerId: string, sequence: string) => ClientServerControlAction,
	): void {
		this.#controlStep(command, (controllerId, sequence) =>
			encodeClientServerRequest({
				action: action(controllerId, sequence.toString()),
				kind: 'terminal.control',
				requestId: createClientServerRequestId(),
			}),
		);
	}

	#controlStep(
		command: ProjectedTerminalCommand,
		encode: (controllerId: string, sequence: bigint) => Uint8Array,
	): void {
		this.#queueControl(command, () => {
			const controllerId = this.#controllerId;
			// Input raced a capability change that the renderer has not applied yet.
			if (controllerId === null) return null;
			return () => encode(controllerId, this.#nextControlSequence++);
		});
	}

	/**
	 * Clear and Reset are screen operations the Server runs for any viewer before it checks
	 * control, so they carry no controller identity or sequence and run without control.
	 */
	#screenAction(command: ProjectedTerminalCommand, kind: 'clear' | 'reset'): void {
		const { attachmentId } = this.#bootstrap;
		this.#queueControl(
			command,
			() => () =>
				encodeClientServerRequest({
					action: { attachmentId, kind },
					kind: 'terminal.control',
					requestId: createClientServerRequestId(),
				}),
		);
	}

	/** Runs one control-lane request in order; `prepare` returns `null` when it no longer applies. */
	#queueControl(command: ProjectedTerminalCommand, prepare: () => (() => Uint8Array) | null): void {
		if (this.#controlSteps.length >= MAXIMUM_QUEUED_COMMANDS) {
			this.#fail(command, 'renderer-backpressure', 'Too many terminal commands are queued.', true);
			return;
		}
		this.#controlSteps.push(async () => {
			if (this.#closed) return;
			const encode = prepare();
			if (encode === null) return;
			try {
				this.#settleControl(command, await this.#submitControl(encode));
			} catch (error) {
				this.#report(command, error);
			}
			this.#pollAfterProgress();
		});
	}

	/** A busy Server consumed any sequence, so each retry encodes again with a new request ID. */
	async #submitControl(encode: () => Uint8Array): Promise<ClientServerControlOutcome> {
		for (const wait of BUSY_RETRY_DELAYS_MILLISECONDS) {
			const outcome = this.#controlOutcome(await this.#exchange(encode()));
			if (outcome.kind !== 'unavailable' || outcome.reason !== 'server-busy') return outcome;
			await delay(wait);
			if (this.#closed) return outcome;
		}
		return this.#controlOutcome(await this.#exchange(encode()));
	}

	#settleControl(command: ProjectedTerminalCommand, outcome: ClientServerControlOutcome): void {
		switch (outcome.kind) {
			case 'rejected':
				// Another Client took over; the renderer only needs to become read-only.
				if (outcome.reason === 'not-controller') {
					this.#setController(null);
				} else if (outcome.reason === 'viewer-unavailable') {
					this.#retire(true);
				} else {
					this.#fail(
						command,
						outcome.reason,
						`The terminal refused this action: ${outcome.reason}.`,
						outcome.reason === 'host-input-backpressure',
					);
				}
				return;
			case 'unavailable':
				this.#fail(command, outcome.reason, `Unavailable: ${outcome.reason}.`, true);
				return;
			case 'outcome-unknown':
				this.#fail(command, 'outcome-unknown', 'The action may or may not have applied.', false);
				return;
			default:
		}
	}

	/**
	 * The renderer requires each capability change to advance its revision by exactly one. Each new
	 * grant reports the window's colors, even one that leaves the capabilities as they were.
	 */
	#setController(controllerId: string | null): void {
		const previous = this.#controllerId;
		const hasControl = controllerId !== null;
		this.#controllerId = controllerId;
		if (controllerId !== previous && hasControl) this.#nextControlSequence = 1n;
		if ((previous !== null) !== hasControl) {
			if (!hasControl) this.#controlSteps.clear();
			this.#capabilityRevision += 1n;
			const { attachmentId, terminalId } = this.#bootstrap;
			this.#post({
				capabilities: {
					attachmentId,
					capabilities: { input: hasControl, read: true, resize: hasControl },
					capabilityRevision: CanonicalUint64TextSchema.parse(this.#capabilityRevision.toString()),
					resizeLease: hasControl,
					terminalId,
				},
				kind: 'attachment-input-capability-changed',
			});
			this.#post({
				event: { attachmentId, resizeLease: hasControl, terminalId },
				kind: 'resize-lease-changed',
			});
		}
		if (hasControl && controllerId !== previous) this.#hostColors?.granted();
	}

	#enqueueHostColors(step: () => Promise<void>): boolean {
		if (
			this.#closed ||
			this.#closing !== null ||
			this.#controlSteps.length >= MAXIMUM_QUEUED_COMMANDS
		) {
			return false;
		}
		this.#controlSteps.push(step);
		return true;
	}

	/** Losing control or the attachment matters here; any other answer settles the colors. */
	async #submitHostColors({
		background,
		foreground,
		cursor,
		palette,
	}: ServerHostColors): Promise<boolean> {
		const controllerId = this.#controllerId;
		if (this.#closed || controllerId === null) return false;
		const { attachmentId } = this.#bootstrap;
		let outcome: ClientServerControlOutcome;
		try {
			outcome = await this.#submitControl(() =>
				encodeClientServerRequest({
					action: {
						attachmentId,
						background,
						controllerId,
						foreground,
						cursor,
						palette,
						kind: 'host-colors',
						sequence: (this.#nextControlSequence++).toString(),
					},
					kind: 'terminal.control',
					requestId: createClientServerRequestId(),
				}),
			);
		} catch {
			return false;
		}
		if (outcome.kind === 'outcome-unknown') return false;
		if (outcome.kind === 'rejected' && outcome.reason === 'not-controller') {
			this.#setController(null);
		} else if (outcome.kind === 'rejected' && outcome.reason === 'viewer-unavailable') {
			this.#retire(true);
		}
		this.#pollAfterProgress();
		return true;
	}

	/**
	 * Hints replace the poll timer once they have proved themselves, except for a viewer owed a full
	 * frame: its hint can arrive before this transport's watch would recognize it.
	 */
	get #pollsFollowHints(): boolean {
		return this.#connection.viewerHintsSeen && !this.#owedFullFrame;
	}

	#emptyPollDelay(): number {
		// A paused viewer receives no frames, so it has nothing to hurry for.
		return this.#paused || this.#pollsFollowHints
			? FALLBACK_POLL_DELAY_MILLISECONDS
			: UNHINTED_POLL_DELAY_MILLISECONDS;
	}

	#schedulePoll(milliseconds: number): void {
		if (this.#closed || this.#pollInFlight || this.#pollTimer !== null) return;
		this.#pollTimer = setTimeout(() => {
			this.#pollTimer = null;
			this.#pollNow();
		}, milliseconds);
	}

	/** Progress can free a record. A viewer on hints hears about it; any other looks for it now. */
	#pollAfterProgress(): void {
		if (!this.#pollsFollowHints) this.#pollNow();
	}

	#pollNow(): void {
		// Poll moves the record, so one taken before anyone subscribed would be lost.
		if (this.#closed || this.#pollInFlight || this.#listener === null) return;
		if (this.#pollTimer !== null) {
			clearTimeout(this.#pollTimer);
			this.#pollTimer = null;
		}
		this.#observeController();
		this.#pollInFlight = true;
		this.#recordReady = false;
		this.#exchange({
			action: { attachmentId: this.#bootstrap.attachmentId, kind: 'poll' },
			kind: 'terminal.viewer',
			requestId: createClientServerRequestId(),
		}).then(
			(response) => {
				this.#pollInFlight = false;
				this.#receivePoll(response);
			},
			() => {
				this.#pollInFlight = false;
				this.#retire(true);
			},
		);
	}

	#receivePoll(response: ClientServerResponse): void {
		if (this.#closed) return;
		if (response.kind === 'terminal.viewer.record') {
			this.#routeRecord(response.hostRecord);
			// The Worker hints again when a Poll leaves anything behind.
			if (this.#recordReady || !this.#pollsFollowHints) this.#pollNow();
			else this.#schedulePoll(FALLBACK_POLL_DELAY_MILLISECONDS);
			return;
		}
		if (response.kind !== 'terminal.viewer.result') {
			this.#retire(true);
			return;
		}
		const { outcome } = response;
		if (outcome.kind === 'empty') {
			if (this.#recordReady) this.#pollNow();
			else this.#schedulePoll(this.#emptyPollDelay());
		} else if (outcome.kind === 'unavailable' && outcome.reason === 'server-busy') {
			this.#schedulePoll(FALLBACK_POLL_DELAY_MILLISECONDS);
		} else {
			this.#retire(true);
		}
	}

	#routeRecord(hostRecord: Uint8Array): void {
		const decoded = decodeProtocolEnvelope(hostRecord);
		if (!decoded.ok) {
			this.#protocolFailure('invalid-host-record', decoded.detail);
			return;
		}
		const envelope = decoded.value;
		switch (envelope.kind) {
			case MessageKind.snapshotFrame:
			case MessageKind.deltaFrame:
				if (envelope.payloadEncoding === PayloadEncoding.terminalFrame) {
					// Before the post: applying this frame can synchronously ask for another full one.
					if (envelope.kind === MessageKind.snapshotFrame) this.#owedFullFrame = false;
					this.#post({ frame: envelope.payload.slice(), kind: 'frame' });
				} else {
					this.#protocolFailure('invalid-host-frame', 'A frame used the wrong encoding.');
				}
				return;
			case MessageKind.historyChanged: {
				const event = decodeControlJson(envelope.payload, HistoryChangedSchema);
				if (event.ok) this.#post({ event: event.value, kind: 'history-changed' });
				else this.#protocolFailure('invalid-history-event', event.detail);
				return;
			}
			case MessageKind.terminalExited: {
				const event = decodeControlJson(envelope.payload, TerminalExitedSchema);
				if (!event.ok) this.#protocolFailure('invalid-terminal-exit', event.detail);
				else if (this.#admitOutcome(event.value)) {
					this.#settle({ event: event.value, kind: 'terminal-exited' });
				}
				return;
			}
			case MessageKind.terminalFailed: {
				const event = decodeControlJson(envelope.payload, TerminalFailedSchema);
				if (!event.ok) this.#protocolFailure('invalid-terminal-failure', event.detail);
				else if (this.#admitOutcome(event.value)) {
					this.#settle({ event: event.value, kind: 'terminal-failed' });
				}
				return;
			}
			case MessageKind.selectionChanged: {
				const event = decodeControlJson(envelope.payload, SelectionChangedSchema);
				if (!event.ok) {
					this.#selectionEventFailure(event.detail);
				} else if (event.value.attachmentId === this.#bootstrap.attachmentId) {
					this.#post({ event: event.value, kind: 'selection-changed' });
				} else {
					this.#selectionEventFailure('A selection event named another attachment.');
				}
				return;
			}
			case MessageKind.terminalBell:
			case MessageKind.terminalClipboardWriteRequest: {
				const refusal = this.#effects.receive(envelope);
				if (refusal !== null) this.#protocolFailure('invalid-host-effect', refusal);
				return;
			}
			case MessageKind.selectionExportChunk:
				this.#selectionCopy.receiveChunk(envelope);
				return;
			case MessageKind.selectionExportFinished:
				this.#selectionCopy.receiveFinished(envelope);
				return;
			default:
				this.#protocolFailure('unsupported-message', `Unexpected Host record ${envelope.kind}.`);
		}
	}

	#admitOutcome(event: {
		readonly finalRevision: string;
		readonly identity: {
			readonly creationId: string;
			readonly hostEpoch: string;
			readonly terminalId: string;
		};
	}): boolean {
		const { identity } = event;
		if (
			identity.creationId !== this.#bootstrap.creationId ||
			identity.hostEpoch !== this.#bootstrap.hostEpoch ||
			identity.terminalId !== this.#bootstrap.terminalId
		) {
			return false;
		}
		const revision = BigInt(event.finalRevision);
		if (this.#latestOutcomeRevision !== null && revision <= this.#latestOutcomeRevision) {
			return false;
		}
		this.#latestOutcomeRevision = revision;
		return true;
	}
}

async function exchangeOnLane(
	connection: ServerConnection,
	laneId: bigint,
	request: ClientServerRequest,
): Promise<ClientServerResponse> {
	return decodeClientServerResponse(
		await connection.exchange(encodeClientServerRequest(request), { laneId }),
	);
}

/** The same refusals the TUI treats as read-only rather than as a failed attach. */
function acquireMeansReadOnly(outcome: ClientServerControlOutcome): boolean {
	return (
		(outcome.kind === 'rejected' &&
			(outcome.reason === 'controller-busy' || outcome.reason === 'host-rejected')) ||
		(outcome.kind === 'unavailable' && outcome.reason === 'terminal-runtime-unavailable')
	);
}

/**
 * Attaches a viewer, tries to take input and resize, and resizes to the caller's geometry. A
 * terminal another Client controls opens read-only. A controlling projection reports the window's
 * colors first when it has any. `onSettled` hears each end of this exact terminal the renderer is
 * told of.
 */
export async function openServerTerminalProjection(
	connection: ServerConnection,
	binding: ServerTerminalBinding,
	geometry: TerminalGeometry,
	hostColors?: ServerHostColorsSource,
	onSettled?: (settlement: ServerTerminalSettlement) => void,
): Promise<ServerTerminalProjection> {
	const laneId = connection.openLane();
	try {
		const attach = await exchangeOnLane(connection, laneId, {
			action: { kind: 'attach-binding', ...binding },
			kind: 'terminal.viewer',
			requestId: createClientServerRequestId(),
		});
		if (attach.kind !== 'terminal.viewer.result') {
			throw new ServerTerminalProjectionError('invalid-attach-result', false);
		}
		const attached = attach.outcome;
		if (attached.kind === 'unavailable') {
			throw new ServerTerminalProjectionError(attached.reason, true);
		}
		if (attached.kind === 'rejected' || attached.kind === 'rows-rejected') {
			throw new ServerTerminalProjectionError(attached.reason, false);
		}
		if (attached.kind !== 'attached') {
			throw new ServerTerminalProjectionError('invalid-attach-result', false);
		}
		const acquire = await exchangeOnLane(connection, laneId, {
			action: { attachmentId: attached.attachmentId, kind: 'acquire' },
			kind: 'terminal.control',
			requestId: createClientServerRequestId(),
		});
		if (acquire.kind !== 'terminal.control.result') {
			throw new ServerTerminalProjectionError('invalid-acquire-result', false);
		}
		const controllerId = acquire.outcome.kind === 'acquired' ? acquire.outcome.controllerId : null;
		if (controllerId === null && !acquireMeansReadOnly(acquire.outcome)) {
			throw new ServerTerminalProjectionError(acquire.outcome.kind, true);
		}
		const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
			attachmentId: attached.attachmentId,
			canInput: controllerId !== null,
			canResize: controllerId !== null,
			// The Worker replaces this on every input record, so only the renderer reads this revision.
			capabilityRevision: '1',
			creationId: attached.creationId,
			hostEpoch: attached.hostEpoch,
			terminalId: attached.hostTerminalId,
			terminalOutcome: null,
			theme: { operationId: null, revision: '0', terminal: UNREAD_BOOTSTRAP_THEME },
		});
		const transport = new ServerProjectedTerminalTransport(
			connection,
			laneId,
			bootstrap,
			controllerId,
			geometry,
			hostColors,
			onSettled,
		);
		if (controllerId !== null) {
			transport.send({
				kind: 'resize-terminal',
				payload: { attachmentId: bootstrap.attachmentId, ...geometry },
			});
		}
		return {
			bootstrap,
			close: () => transport.close(),
			activateFocus: (initial, signal) => transport.activateFocus(initial, signal),
			transport,
		};
	} catch (error) {
		connection.closeLane(laneId);
		throw error;
	}
}
