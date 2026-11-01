/**
 * Opens one Server terminal as a renderer projection. The viewer and controller share a lane, so
 * closing the lane releases both. The Server has no output-ready signal, so frames are pulled by
 * polling. This holds no terminal semantics; the Host and Worker own those.
 */
import {
	CanonicalUint64TextSchema,
	HistoryChangedSchema,
	MessageKind,
	PayloadEncoding,
	type TerminalGeometry,
	TerminalExitedSchema,
	TerminalFailedSchema,
	type TerminalThemeState,
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

import type { ProjectedTerminalTransport, TerminalProjection } from '../terminal-projection';
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

export type ServerTerminalTarget =
	| {
			readonly kind: 'binding';
			readonly sessionId: string;
			readonly terminalId: string;
			readonly workerId: string;
	  }
	| { readonly kind: 'created'; readonly terminalCreateRequestId: string };

/** A refusal while opening a projection, in the renderer's failure vocabulary. */
class ServerTerminalProjectionError extends Error {
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
	/** Takes input and resize from whichever Client holds them. */
	takeControl(): Promise<boolean>;
}

/** Empty-poll pacing: the TUI's pace while something happened recently, then easing off. */
const ACTIVE_POLL_DELAY_MILLISECONDS = 8;
const IDLE_POLL_DELAY_MILLISECONDS = 33;
const PAUSED_POLL_DELAY_MILLISECONDS = 250;
const ACTIVE_WINDOW_MILLISECONDS = 1_000;
const BUSY_RETRY_DELAYS_MILLISECONDS = [4, 8, 16, 32, 64, 128, 256] as const;
const MINIMUM_ROW_READ_BYTES = 112;
const MAXIMUM_ROW_READ_BYTES = 8 * 1024 * 1024;
const MAXIMUM_QUEUED_COMMANDS = 256;

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

	async #run(): Promise<void> {
		this.#running = true;
		for (let step = this.#queue.shift(); step !== undefined; step = this.#queue.shift()) {
			await step();
		}
		this.#running = false;
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
		case 'image-preview':
		case 'image-preview-cancel':
			return { operation: 'protocol', requestSequence: command.payload.requestSequence };
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
	readonly #laneId: bigint;
	readonly #viewerSteps = new SerialSteps();
	readonly #unsubscribeEnded: () => void;
	#capabilityRevision = 1n;
	#closed = false;
	#controllerId: string | null;
	#lastActivity = 0;
	#latestOutcomeRevision: bigint | null = null;
	#listener: ((event: ProjectedTerminalEvent) => void) | null = null;
	#nextControlSequence = 1n;
	#paused = false;
	#pollInFlight = false;
	#pollTimer: ReturnType<typeof setTimeout> | null = null;
	#unavailable: (() => void) | null = null;

	constructor(
		connection: ServerConnection,
		laneId: bigint,
		bootstrap: ProjectedAttachmentBootstrap,
		controllerId: string | null,
	) {
		this.#bootstrap = bootstrap;
		this.#connection = connection;
		this.#controllerId = controllerId;
		this.#laneId = laneId;
		this.#unsubscribeEnded = connection.onEnded(() => this.#retire(true));
		if (controllerId !== null) this.#reportHostColors();
	}

	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void {
		this.#listener = listener;
		this.#unavailable = unavailable;
		this.#pollNow();
		return () => {
			this.#listener = null;
			this.#unavailable = null;
		};
	}

	send(command: ProjectedTerminalCommand): void {
		if (this.#closed) return;
		const attachmentId = this.#bootstrap.attachmentId;
		switch (command.kind) {
			case 'frame-applied':
				this.#viewerStep(command, { ...command.payload, kind: 'frame-applied' }, true);
				return;
			case 'need-snapshot':
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
				this.#controlAction(command, () => ({ attachmentId, kind: 'clear' }));
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

	close(): void {
		this.#retire(false);
	}

	async takeControl(): Promise<boolean> {
		if (this.#closed) return false;
		const outcome = this.#controlOutcome(
			await this.#exchange({
				action: { attachmentId: this.#bootstrap.attachmentId, kind: 'takeover' },
				kind: 'terminal.control',
				requestId: createClientServerRequestId(),
			}),
		);
		if (outcome.kind !== 'acquired' || this.#closed) return false;
		this.#setController(outcome.controllerId);
		return true;
	}

	#retire(notify: boolean): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#unsubscribeEnded();
		if (this.#pollTimer !== null) clearTimeout(this.#pollTimer);
		this.#pollTimer = null;
		this.#viewerSteps.clear();
		this.#controlSteps.clear();
		this.#connection.closeLane(this.#laneId);
		const unavailable = this.#unavailable;
		this.#unavailable = null;
		if (notify) unavailable?.();
	}

	#post(event: ProjectedTerminalEvent): void {
		if (!this.#closed) this.#listener?.(event);
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

	async #viewer(action: ClientServerViewerAction): Promise<ClientServerViewerOutcome> {
		const response = await this.#exchange({
			action,
			kind: 'terminal.viewer',
			requestId: createClientServerRequestId(),
		});
		if (response.kind !== 'terminal.viewer.result') {
			throw new Error('The Server answered a viewer action with another message.');
		}
		return response.outcome;
	}

	#viewerStep(
		command: ProjectedTerminalCommand,
		action: ClientServerViewerAction,
		wakesPoll: boolean,
	): void {
		if (this.#viewerSteps.length >= MAXIMUM_QUEUED_COMMANDS) {
			this.#fail(command, 'renderer-backpressure', 'Too many terminal commands are queued.', true);
			return;
		}
		this.#viewerSteps.push(async () => {
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
			if (wakesPoll) this.#pollNow();
		});
	}

	async #viewerWithRetry(action: ClientServerViewerAction): Promise<ClientServerViewerOutcome> {
		for (const wait of BUSY_RETRY_DELAYS_MILLISECONDS) {
			const outcome = await this.#viewer(action);
			if (outcome.kind !== 'unavailable' || outcome.reason !== 'server-busy') return outcome;
			await delay(wait);
			if (this.#closed) return outcome;
		}
		return this.#viewer(action);
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
		} else if (payload.origin.kind === 'absolute' && payload.purpose !== 'viewport') {
			action = { ...shared, kind: 'read-rows', startRow: payload.origin.row };
		} else {
			// The Worker only pins a viewport for tail and committed reads, so an absolute viewport
			// read could not be acknowledged by the rows-applied that follows it.
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

	#reportHostColors(): void {
		const { defaultBackgroundRgb, defaultForegroundRgb } = this.#bootstrap.theme.terminal;
		this.#controlSteps.push(async () => {
			const controllerId = this.#controllerId;
			if (this.#closed || controllerId === null) return;
			try {
				await this.#submitControl(controllerId, (_controller, sequence) =>
					encodeClientServerRequest({
						action: {
							attachmentId: this.#bootstrap.attachmentId,
							background: defaultBackgroundRgb,
							controllerId,
							foreground: defaultForegroundRgb,
							kind: 'host-colors',
							sequence: sequence.toString(),
						},
						kind: 'terminal.control',
						requestId: createClientServerRequestId(),
					}),
				);
			} catch {
				// Host colors only refine color queries; a failure leaves input working.
			}
		});
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
		if (this.#controlSteps.length >= MAXIMUM_QUEUED_COMMANDS) {
			this.#fail(command, 'renderer-backpressure', 'Too many terminal commands are queued.', true);
			return;
		}
		// The user is waiting to see this take effect, so the next poll should not wait on idle pacing.
		this.#lastActivity = performance.now();
		this.#controlSteps.push(async () => {
			const controllerId = this.#controllerId;
			// Input raced a capability change that the renderer has not applied yet.
			if (this.#closed || controllerId === null) return;
			try {
				this.#settleControl(command, await this.#submitControl(controllerId, encode));
			} catch (error) {
				this.#report(command, error);
			}
			this.#pollSoon();
		});
	}

	/** A busy Server consumed the sequence, so each retry takes a new one and a new request ID. */
	async #submitControl(
		controllerId: string,
		encode: (controllerId: string, sequence: bigint) => Uint8Array,
	): Promise<ClientServerControlOutcome> {
		for (const wait of BUSY_RETRY_DELAYS_MILLISECONDS) {
			const outcome = this.#controlOutcome(
				await this.#exchange(encode(controllerId, this.#nextControlSequence++)),
			);
			if (outcome.kind !== 'unavailable' || outcome.reason !== 'server-busy') return outcome;
			await delay(wait);
			if (this.#closed) return outcome;
		}
		return this.#controlOutcome(
			await this.#exchange(encode(controllerId, this.#nextControlSequence++)),
		);
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

	/** The renderer requires each capability change to advance its revision by exactly one. */
	#setController(controllerId: string | null): void {
		const hadControl = this.#controllerId !== null;
		const hasControl = controllerId !== null;
		this.#controllerId = controllerId;
		if (hadControl === hasControl) return;
		if (hasControl) this.#reportHostColors();
		else this.#controlSteps.clear();
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

	#pollDelay(): number {
		if (performance.now() - this.#lastActivity < ACTIVE_WINDOW_MILLISECONDS) {
			return ACTIVE_POLL_DELAY_MILLISECONDS;
		}
		return this.#paused ? PAUSED_POLL_DELAY_MILLISECONDS : IDLE_POLL_DELAY_MILLISECONDS;
	}

	#schedulePoll(milliseconds: number): void {
		if (this.#closed || this.#pollInFlight || this.#pollTimer !== null) return;
		this.#pollTimer = setTimeout(() => {
			this.#pollTimer = null;
			this.#pollNow();
		}, milliseconds);
	}

	#pollSoon(): void {
		if (this.#pollTimer !== null) {
			clearTimeout(this.#pollTimer);
			this.#pollTimer = null;
		}
		this.#schedulePoll(ACTIVE_POLL_DELAY_MILLISECONDS);
	}

	#pollNow(): void {
		if (this.#closed || this.#pollInFlight) return;
		if (this.#pollTimer !== null) {
			clearTimeout(this.#pollTimer);
			this.#pollTimer = null;
		}
		this.#pollInFlight = true;
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
			this.#lastActivity = performance.now();
			this.#routeRecord(response.hostRecord);
			this.#pollNow();
			return;
		}
		if (response.kind !== 'terminal.viewer.result') {
			this.#retire(true);
			return;
		}
		const { outcome } = response;
		if (outcome.kind === 'empty') {
			this.#schedulePoll(this.#pollDelay());
		} else if (outcome.kind === 'unavailable' && outcome.reason === 'server-busy') {
			this.#schedulePoll(PAUSED_POLL_DELAY_MILLISECONDS);
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
					this.#post({ event: event.value, kind: 'terminal-exited' });
				}
				return;
			}
			case MessageKind.terminalFailed: {
				const event = decodeControlJson(envelope.payload, TerminalFailedSchema);
				if (!event.ok) this.#protocolFailure('invalid-terminal-failure', event.detail);
				else if (this.#admitOutcome(event.value)) {
					this.#post({ event: event.value, kind: 'terminal-failed' });
				}
				return;
			}
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

/** Starts a terminal for this connection and returns the identity that attaches a viewer to it. */
export async function createServerTerminal(
	connection: ServerConnection,
	geometry: TerminalGeometry,
): Promise<string> {
	const requestId = createClientServerRequestId();
	const response = decodeClientServerResponse(
		await connection.exchange(
			encodeClientServerRequest({
				cwd: null,
				kind: 'terminal.create',
				requestId,
				startMode: { geometry, kind: 'immediate' },
				workerConstraint: null,
			}),
		),
	);
	if (response.kind !== 'terminal.create.result') {
		throw new ServerTerminalProjectionError('invalid-create-result', false);
	}
	const { outcome } = response;
	if (outcome.kind === 'accepted') return requestId;
	if (outcome.kind === 'outcome-unknown') {
		throw new ServerTerminalProjectionError('outcome-unknown', false);
	}
	throw new ServerTerminalProjectionError(outcome.reason, outcome.kind === 'unavailable');
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
 * terminal another Client controls opens read-only.
 */
export async function openServerTerminalProjection(
	connection: ServerConnection,
	target: ServerTerminalTarget,
	geometry: TerminalGeometry,
	theme: TerminalThemeState['terminal'],
): Promise<ServerTerminalProjection> {
	const laneId = connection.openLane();
	try {
		const attach = await exchangeOnLane(connection, laneId, {
			action:
				target.kind === 'created'
					? { kind: 'attach', terminalCreateRequestId: target.terminalCreateRequestId }
					: {
							kind: 'attach-binding',
							sessionId: target.sessionId,
							terminalId: target.terminalId,
							workerId: target.workerId,
						},
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
			theme: { operationId: null, revision: '0', terminal: theme },
		});
		const transport = new ServerProjectedTerminalTransport(
			connection,
			laneId,
			bootstrap,
			controllerId,
		);
		if (controllerId !== null) {
			transport.send({
				kind: 'resize-terminal',
				payload: { attachmentId: bootstrap.attachmentId, ...geometry },
			});
		}
		return { bootstrap, takeControl: () => transport.takeControl(), transport };
	} catch (error) {
		connection.closeLane(laneId);
		throw error;
	}
}
