import type { MessageEvent, MessagePortMain } from 'electron';

import {
	ProjectedTerminalCommandSchema,
	type ProjectedAttachmentBootstrap,
	type ProjectedTerminalCommand,
	type ProjectedTerminalFailure,
} from '@orchbee/workbench-client-protocol';

import {
	AttachmentAcknowledgedSchema,
	CanonicalUint64TextSchema,
	decodeControlJson,
	decodeSelectionExportChunk,
	encodeFocusInput,
	encodeKeyInput,
	encodeMouseInput,
	encodePasteInput,
	encodeTextInput,
	HistoryChangedSchema,
	HistoryDemandAcceptedSchema,
	HistoryObservedSchema,
	MessageKind,
	PayloadEncoding,
	ProjectionBlockedSchema,
	PROTOCOL_LIMITS,
	RowsAppliedSchema,
	SelectionChangedSchema,
	SelectionExportChunkAppliedSchema,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	TerminalExitedSchema,
	type DecodedProtocolEnvelope,
	type MessageKindValue,
	type SelectionExportChunk,
	type SelectionExportCancel,
} from '@orchbee/terminal-protocol';
import type { HostConnection } from '../host';
import { HostRequestError } from '../host';

const MAX_IN_FLIGHT_RENDERER_REQUESTS = 64;
const textEncoder = new TextEncoder();
const fatalTextDecoder = new TextDecoder('utf-8', { fatal: true });

interface ActiveSelectionCopy {
	buffer: Uint8Array | undefined;
	byteLength: number | undefined;
	committing: boolean;
	earlyChunk: SelectionExportChunk | undefined;
	exportSequence: ReturnType<typeof CanonicalUint64TextSchema.parse>;
	finalReceived: boolean;
	nextChunkSequence: number;
	offset: number;
	requestSequence: ReturnType<typeof CanonicalUint64TextSchema.parse>;
	startedAtMilliseconds: number;
}

export interface ProjectedTerminalBrokerDependencies {
	readonly openExternal: (target: string) => Promise<void>;
	readonly writeClipboardText: (text: string) => Promise<void> | void;
}

export type ProjectedTerminalBrokerEffectTarget = 'client' | 'desktop';

const defaultDependencies: ProjectedTerminalBrokerDependencies = {
	openExternal: async (target) => {
		const { shell } = await import('electron');
		await shell.openExternal(target);
	},
	writeClipboardText: async (text) => {
		const { clipboard } = await import('electron');
		clipboard.writeText(text);
	},
};

class BrokerOperationError extends Error {
	readonly code: string;
	readonly retryable: boolean;

	constructor(code: string, message: string, retryable = false) {
		super(message);
		this.name = 'BrokerOperationError';
		this.code = code;
		this.retryable = retryable;
	}
}

export interface ProjectedTerminalBrokerMetrics {
	readonly clipboardCommits: number;
	readonly commandsAccepted: number;
	readonly commandsRejected: number;
	readonly externalTargetsOpened: number;
	readonly frameBytesForwarded: number;
	readonly framesForwarded: number;
	readonly lastSelectionCopyMilliseconds: number | null;
	readonly lastSelectionExportStartMilliseconds: number | null;
	readonly rowWindowBytesForwarded: number;
	readonly rowWindowsForwarded: number;
	readonly selectionExportBytesForwarded: number;
	readonly selectionExportChunksForwarded: number;
}

export class ProjectedTerminalBroker {
	readonly #bootstrap: ProjectedAttachmentBootstrap;
	readonly #dependencies: ProjectedTerminalBrokerDependencies;
	readonly #effectTarget: ProjectedTerminalBrokerEffectTarget;
	readonly #host: HostConnection;
	readonly #port: MessagePortMain;
	#activeSelectionCopy: ActiveSelectionCopy | undefined;
	#clipboardCommits = 0;
	#commandsAccepted = 0;
	#commandsRejected = 0;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	#externalTargetsOpened = 0;
	#frameBytesForwarded = 0;
	#framesForwarded = 0;
	#inFlightRequests = 0;
	#lastSelectionCopyMilliseconds: number | null = null;
	#lastSelectionExportStartMilliseconds: number | null = null;
	#nextExportSequence = 1n;
	#removeHostListener: (() => void) | undefined;
	#rowWindowBytesForwarded = 0;
	#rowWindowsForwarded = 0;
	#selectionExportBytesForwarded = 0;
	#selectionExportChunksForwarded = 0;

	constructor(
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
		dependencies: ProjectedTerminalBrokerDependencies = defaultDependencies,
		effectTarget: ProjectedTerminalBrokerEffectTarget = 'desktop',
	) {
		this.#host = host;
		this.#port = port;
		this.#bootstrap = bootstrap;
		this.#dependencies = dependencies;
		this.#effectTarget = effectTarget;
	}

	get metrics(): ProjectedTerminalBrokerMetrics {
		return {
			clipboardCommits: this.#clipboardCommits,
			commandsAccepted: this.#commandsAccepted,
			commandsRejected: this.#commandsRejected,
			externalTargetsOpened: this.#externalTargetsOpened,
			frameBytesForwarded: this.#frameBytesForwarded,
			framesForwarded: this.#framesForwarded,
			lastSelectionCopyMilliseconds: this.#lastSelectionCopyMilliseconds,
			lastSelectionExportStartMilliseconds: this.#lastSelectionExportStartMilliseconds,
			rowWindowBytesForwarded: this.#rowWindowBytesForwarded,
			rowWindowsForwarded: this.#rowWindowsForwarded,
			selectionExportBytesForwarded: this.#selectionExportBytesForwarded,
			selectionExportChunksForwarded: this.#selectionExportChunksForwarded,
		};
	}

	start(): void {
		if (this.#disposed || this.#removeHostListener !== undefined) return;
		const removeFrameListener = this.#host.onAttachmentEnvelope(
			this.#bootstrap.attachmentId,
			this.#handleHostEnvelope,
		);
		const removeControlListener = this.#host.onEnvelope(this.#handleHostEnvelope);
		this.#removeHostListener = () => {
			removeFrameListener();
			removeControlListener();
		};
		this.#port.on('message', this.#handleRendererMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	dispose(): Promise<void> {
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		const activeCopy = this.#activeSelectionCopy;
		this.#activeSelectionCopy = undefined;
		this.#disposed = true;
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		this.#port.off('message', this.#handleRendererMessage);
		this.#port.off('close', this.#handlePortClose);
		this.#port.close();
		this.#disposePromise = this.#detach(activeCopy);
		return this.#disposePromise;
	}

	readonly #handlePortClose = (): void => {
		void this.dispose();
	};

	async #detach(activeCopy: ActiveSelectionCopy | undefined): Promise<void> {
		if (activeCopy !== undefined) {
			await this.#requestSelectionExportCancel(activeCopy, 'disposed').catch(() => undefined);
		}
		await this.#host
			.requestControl(
				MessageKind.detachTerminal,
				{ attachmentId: this.#bootstrap.attachmentId },
				MessageKind.detachTerminal,
				AttachmentAcknowledgedSchema,
			)
			.catch(() => undefined);
	}

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const rawMessage: unknown = event.data;
		const parsed = ProjectedTerminalCommandSchema.safeParse(rawMessage);
		if (!parsed.success || parsed.data.payload.attachmentId !== this.#bootstrap.attachmentId) {
			this.#commandsRejected += 1;
			this.#postFailure({
				code: 'renderer-capability-rejected',
				message: 'Renderer command is invalid or outside its attachment capability.',
				retryable: false,
			});
			return;
		}
		if (this.#inFlightRequests >= MAX_IN_FLIGHT_RENDERER_REQUESTS) {
			this.#commandsRejected += 1;
			this.#postFailure({
				code: 'renderer-backpressure',
				message: 'Renderer command exceeded the bounded Main request window.',
				retryable: true,
			});
			return;
		}

		this.#commandsAccepted += 1;
		this.#inFlightRequests += 1;
		void this.#dispatch(parsed.data)
			.catch((error: unknown) => {
				const context = failureContext(parsed.data);
				this.#postFailure(
					error instanceof HostRequestError || error instanceof BrokerOperationError
						? {
								code: error.code,
								message: error.message,
								retryable: error.retryable,
								...context,
							}
						: {
								code: 'host-request-failed',
								message: error instanceof Error ? error.message : 'Terminal Host request failed.',
								retryable: false,
								...context,
							},
				);
			})
			.finally(() => {
				this.#inFlightRequests -= 1;
			});
	};

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (this.#disposed) return;
		switch (envelope.kind) {
			case MessageKind.historyChanged: {
				const event = decodeControlJson(envelope.payload, HistoryChangedSchema);
				if (!event.ok || event.value.attachmentId !== this.#bootstrap.attachmentId) {
					this.#postFailure({
						code: 'invalid-history-event',
						message: event.ok
							? 'Terminal Host history event is outside the scoped attachment.'
							: event.detail,
						retryable: false,
					});
					return;
				}
				this.#port.postMessage({ event: event.value, kind: 'history-changed' });
				return;
			}
			case MessageKind.selectionChanged: {
				const event = decodeControlJson(envelope.payload, SelectionChangedSchema);
				if (!event.ok || event.value.attachmentId !== this.#bootstrap.attachmentId) {
					this.#postFailure({
						code: 'invalid-selection-event',
						message: event.ok
							? 'Terminal Host selection event is outside the scoped attachment.'
							: event.detail,
						retryable: false,
					});
					return;
				}
				this.#port.postMessage({ event: event.value, kind: 'selection-changed' });
				return;
			}
			case MessageKind.selectionExportChunk:
				void this.#receiveSelectionExportChunk(envelope).catch((error: unknown) => {
					this.#failActiveSelectionCopy(error, 'replaced');
				});
				return;
			case MessageKind.selectionExportFinished:
				void this.#finishSelectionCopy(envelope).catch((error: unknown) => {
					this.#failActiveSelectionCopy(error);
				});
				return;
			case MessageKind.snapshotFrame:
			case MessageKind.deltaFrame: {
				if (envelope.payloadEncoding !== PayloadEncoding.terminalFrame) {
					this.#postFailure({
						code: 'invalid-host-frame',
						message: 'Terminal Host frame used the wrong payload encoding.',
						retryable: false,
					});
					return;
				}
				const frameBytes = new Uint8Array(envelope.payload.byteLength);
				frameBytes.set(envelope.payload);
				this.#framesForwarded += 1;
				this.#frameBytesForwarded += frameBytes.byteLength;
				this.#port.postMessage({ frame: frameBytes.buffer, kind: 'frame' });
				return;
			}
			case MessageKind.terminalExited: {
				const event = decodeControlJson(envelope.payload, TerminalExitedSchema);
				if (!event.ok) {
					this.#postFailure({
						code: 'invalid-terminal-exit',
						message: event.detail,
						retryable: false,
					});
					return;
				}
				if (
					event.value.identity.creationId !== this.#bootstrap.creationId ||
					event.value.identity.hostEpoch !== this.#bootstrap.hostEpoch ||
					event.value.identity.terminalId !== this.#bootstrap.terminalId
				) {
					return;
				}
				this.#port.postMessage({ event: event.value, kind: 'terminal-exited' });
				return;
			}
			case MessageKind.projectionBlocked: {
				const blocked = decodeControlJson(envelope.payload, ProjectionBlockedSchema);
				if (blocked.ok && blocked.value.attachmentId !== this.#bootstrap.attachmentId) return;
				this.#postFailure({
					code: 'projection-blocked',
					message: blocked.ok
						? `Terminal projection is blocked: ${blocked.value.reason}.`
						: blocked.detail,
					retryable: false,
				});
				return;
			}
			default:
				return;
		}
	};

	async #dispatch(command: ProjectedTerminalCommand): Promise<void> {
		switch (command.kind) {
			case 'focus-input':
				return this.#requestEncoded(
					MessageKind.focusInput,
					encodeFocusInput(command.payload),
					MessageKind.inputAccepted,
				);
			case 'frame-applied':
				await this.#host.request(
					MessageKind.frameApplied,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.frameApplied,
				);
				return;
			case 'key-input':
				return this.#requestEncoded(
					MessageKind.keyInput,
					encodeKeyInput(command.payload),
					MessageKind.inputAccepted,
				);
			case 'mouse-input':
				return this.#requestEncoded(
					MessageKind.mouseInput,
					encodeMouseInput(command.payload),
					MessageKind.inputAccepted,
				);
			case 'need-snapshot':
				await this.#host.request(
					MessageKind.needSnapshot,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.needSnapshot,
				);
				return;
			case 'paste-input':
				return this.#requestEncoded(
					MessageKind.pasteInput,
					encodePasteInput(command.payload),
					MessageKind.inputAccepted,
				);
			case 'read-rows': {
				const response = await this.#host.request(
					MessageKind.readRows,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.rowsSnapshot,
				);
				if (
					response.payloadEncoding !== PayloadEncoding.rowWindow ||
					response.payload.byteLength > command.payload.maximumBytes ||
					response.payload.byteLength > PROTOCOL_LIMITS.maxRowWindowBytes
				) {
					throw new BrokerOperationError(
						'invalid-row-window',
						'Terminal Host returned an invalid or oversized row window.',
					);
				}
				const rows = new Uint8Array(response.payload.byteLength);
				rows.set(response.payload);
				this.#rowWindowsForwarded += 1;
				this.#rowWindowBytesForwarded += rows.byteLength;
				this.#port.postMessage({ kind: 'rows', rows: rows.buffer });
				return;
			}
			case 'resize-terminal':
				await this.#host.request(
					MessageKind.resizeTerminal,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.resizeAccepted,
				);
				return;
			case 'rows-applied':
				await this.#host.requestControl(
					MessageKind.rowsApplied,
					command.payload,
					MessageKind.rowsApplied,
					RowsAppliedSchema,
				);
				return;
			case 'history-observed':
				await this.#host.requestControl(
					MessageKind.historyObserved,
					command.payload,
					MessageKind.historyObserved,
					HistoryObservedSchema,
				);
				return;
			case 'selection-command':
			case 'selection-gesture': {
				const kind =
					command.kind === 'selection-command'
						? MessageKind.selectionCommand
						: MessageKind.selectionGesture;
				const selection = await this.#host.requestControl(
					kind,
					command.payload,
					MessageKind.selectionChanged,
					SelectionChangedSchema,
				);
				if (selection.attachmentId !== this.#bootstrap.attachmentId) {
					throw new BrokerOperationError(
						'invalid-selection-response',
						'Terminal Host selection response is outside the scoped attachment.',
					);
				}
				this.#port.postMessage({ event: selection, kind: 'selection-changed' });
				return;
			}
			case 'selection-copy':
				await this.#startSelectionCopy(command.payload.requestSequence);
				return;
			case 'set-history-demand':
				await this.#host.requestControl(
					MessageKind.setHistoryDemand,
					command.payload,
					MessageKind.historyDemandAccepted,
					HistoryDemandAcceptedSchema,
				);
				return;
			case 'set-projection-demand':
				await this.#host.request(
					MessageKind.setProjectionDemand,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.projectionDemandAccepted,
				);
				return;
			case 'link-open':
				if (this.#effectTarget === 'client') {
					throw new BrokerOperationError(
						'link-client-effect-required',
						'This attachment requires its client environment to open links.',
					);
				}
				await this.#openExternal(command.payload.target);
				this.#port.postMessage({
					kind: 'link-opened',
					requestSequence: command.payload.requestSequence,
				});
				return;
			case 'text-input':
				return this.#requestEncoded(
					MessageKind.textInput,
					encodeTextInput(command.payload),
					MessageKind.inputAccepted,
				);
		}
	}

	async #startSelectionCopy(
		requestSequence: ReturnType<typeof CanonicalUint64TextSchema.parse>,
	): Promise<void> {
		if (this.#activeSelectionCopy !== undefined) {
			throw new BrokerOperationError(
				'copy-in-progress',
				'A terminal selection copy is already in progress.',
				true,
			);
		}
		if (this.#nextExportSequence > 0xffff_ffff_ffff_ffffn) {
			this.#nextExportSequence = 1n;
		}
		const exportSequence = CanonicalUint64TextSchema.parse((this.#nextExportSequence++).toString());
		const state: ActiveSelectionCopy = {
			buffer: undefined,
			byteLength: undefined,
			committing: false,
			earlyChunk: undefined,
			exportSequence,
			finalReceived: false,
			nextChunkSequence: 0,
			offset: 0,
			requestSequence,
			startedAtMilliseconds: performance.now(),
		};
		this.#activeSelectionCopy = state;
		try {
			const started = await this.#host.requestControl(
				MessageKind.selectionExport,
				{
					attachmentId: this.#bootstrap.attachmentId,
					exportSequence,
					format: 'plain',
				},
				MessageKind.selectionExportStarted,
				SelectionExportStartedSchema,
			);
			this.#lastSelectionExportStartMilliseconds = performance.now() - state.startedAtMilliseconds;
			if (
				started.attachmentId !== this.#bootstrap.attachmentId ||
				started.exportSequence !== exportSequence ||
				started.format !== 'plain'
			) {
				throw new BrokerOperationError(
					'invalid-copy-export',
					'Terminal Host started an export outside the scoped copy request.',
				);
			}
			state.byteLength = started.byteLength;
			state.buffer = new Uint8Array(started.byteLength);
			const earlyChunk = state.earlyChunk;
			state.earlyChunk = undefined;
			if (earlyChunk !== undefined) await this.#applySelectionExportChunk(state, earlyChunk);
		} catch (error) {
			if (this.#activeSelectionCopy === state) this.#activeSelectionCopy = undefined;
			throw error;
		}
	}

	async #receiveSelectionExportChunk(envelope: DecodedProtocolEnvelope): Promise<void> {
		if (envelope.payloadEncoding !== PayloadEncoding.rawBytes) {
			throw new BrokerOperationError(
				'invalid-copy-chunk',
				'Terminal Host selection export used the wrong payload encoding.',
			);
		}
		const decoded = decodeSelectionExportChunk(envelope.payload);
		if (!decoded.ok) {
			throw new BrokerOperationError('invalid-copy-chunk', decoded.detail);
		}
		const state = this.#activeSelectionCopy;
		if (
			state === undefined ||
			decoded.value.attachmentId !== this.#bootstrap.attachmentId ||
			decoded.value.exportSequence.toString() !== state.exportSequence
		) {
			throw new BrokerOperationError(
				'invalid-copy-chunk',
				'Terminal Host selection export chunk is outside the active copy request.',
			);
		}
		if (state.buffer === undefined) {
			if (state.earlyChunk !== undefined) {
				throw new BrokerOperationError(
					'invalid-copy-chunk',
					'Terminal Host emitted more than one unacknowledged selection chunk.',
				);
			}
			state.earlyChunk = decoded.value;
			return;
		}
		await this.#applySelectionExportChunk(state, decoded.value);
	}

	async #applySelectionExportChunk(
		state: ActiveSelectionCopy,
		chunk: SelectionExportChunk,
	): Promise<void> {
		const buffer = state.buffer;
		if (
			buffer === undefined ||
			chunk.chunkSequence !== state.nextChunkSequence ||
			chunk.bytes.byteLength > PROTOCOL_LIMITS.maxSelectionExportChunkBytes ||
			state.offset + chunk.bytes.byteLength > buffer.byteLength ||
			state.finalReceived
		) {
			throw new BrokerOperationError(
				'invalid-copy-chunk',
				'Terminal Host emitted a non-canonical selection export chunk.',
			);
		}
		buffer.set(chunk.bytes, state.offset);
		this.#selectionExportChunksForwarded += 1;
		this.#selectionExportBytesForwarded += chunk.bytes.byteLength;
		state.offset += chunk.bytes.byteLength;
		state.nextChunkSequence += 1;
		state.finalReceived = chunk.final;
		if (chunk.final && state.offset !== buffer.byteLength) {
			throw new BrokerOperationError(
				'invalid-copy-chunk',
				'Terminal Host ended a selection export before its declared byte length.',
			);
		}
		await this.#host.requestControl(
			MessageKind.selectionExportChunkApplied,
			{
				attachmentId: this.#bootstrap.attachmentId,
				chunkSequence: chunk.chunkSequence,
				exportSequence: state.exportSequence,
			},
			MessageKind.selectionExportChunkApplied,
			SelectionExportChunkAppliedSchema,
		);
	}

	async #finishSelectionCopy(envelope: DecodedProtocolEnvelope): Promise<void> {
		if (envelope.payloadEncoding !== PayloadEncoding.controlJson) {
			throw new BrokerOperationError(
				'invalid-copy-finish',
				'Terminal Host selection export completion used the wrong payload encoding.',
			);
		}
		const decoded = decodeControlJson(envelope.payload, SelectionExportFinishedSchema);
		if (!decoded.ok) throw new BrokerOperationError('invalid-copy-finish', decoded.detail);
		const state = this.#activeSelectionCopy;
		if (
			state === undefined ||
			decoded.value.attachmentId !== this.#bootstrap.attachmentId ||
			decoded.value.exportSequence !== state.exportSequence
		) {
			throw new BrokerOperationError(
				'invalid-copy-finish',
				'Terminal Host selection export completion is outside the active copy request.',
			);
		}
		if (state.committing) return;
		state.committing = true;
		if (
			decoded.value.status !== 'completed' ||
			state.buffer === undefined ||
			!state.finalReceived ||
			state.offset !== state.buffer.byteLength ||
			decoded.value.byteLength !== state.buffer.byteLength
		) {
			throw new BrokerOperationError(
				'copy-cancelled',
				'Terminal selection export did not complete.',
				decoded.value.status === 'cancelled',
			);
		}
		let text: string;
		try {
			text = fatalTextDecoder.decode(state.buffer);
		} catch {
			throw new BrokerOperationError(
				'invalid-copy-utf8',
				'Terminal Host selection export is not valid UTF-8.',
			);
		}
		const byteLength = state.buffer.byteLength;
		if (this.#effectTarget === 'desktop') {
			await this.#dependencies.writeClipboardText(text);
			this.#clipboardCommits += 1;
		} else {
			const content = new Uint8Array(byteLength);
			content.set(state.buffer);
			this.#port.postMessage({
				content: content.buffer,
				kind: 'selection-copy-content',
				requestSequence: state.requestSequence,
			});
		}
		if (this.#activeSelectionCopy !== state) return;
		this.#lastSelectionCopyMilliseconds = performance.now() - state.startedAtMilliseconds;
		this.#activeSelectionCopy = undefined;
		if (this.#effectTarget === 'desktop') {
			this.#port.postMessage({
				byteLength,
				kind: 'selection-copied',
				requestSequence: state.requestSequence,
			});
		}
	}

	async #openExternal(target: string): Promise<void> {
		let parsed: URL;
		try {
			parsed = new URL(target);
		} catch {
			throw new BrokerOperationError('link-policy-blocked', 'The terminal link is not a URL.');
		}
		if (
			parsed.protocol !== 'http:' &&
			parsed.protocol !== 'https:' &&
			parsed.protocol !== 'mailto:'
		) {
			throw new BrokerOperationError(
				'link-policy-blocked',
				`The terminal link scheme ${parsed.protocol} is not permitted.`,
			);
		}
		await this.#dependencies.openExternal(parsed.href);
		this.#externalTargetsOpened += 1;
	}

	#failActiveSelectionCopy(error: unknown, cancelReason?: SelectionExportCancel['reason']): void {
		const state = this.#activeSelectionCopy;
		this.#activeSelectionCopy = undefined;
		if (state !== undefined && cancelReason !== undefined) {
			void this.#requestSelectionExportCancel(state, cancelReason).catch(() => undefined);
		}
		this.#postFailure(
			error instanceof BrokerOperationError
				? {
						code: error.code,
						message: error.message,
						operation: 'copy',
						...(state === undefined ? {} : { requestSequence: state.requestSequence }),
						retryable: error.retryable,
					}
				: {
						code: 'copy-failed',
						message: error instanceof Error ? error.message : 'Terminal selection copy failed.',
						operation: 'copy',
						...(state === undefined ? {} : { requestSequence: state.requestSequence }),
						retryable: false,
					},
		);
	}

	async #requestSelectionExportCancel(
		state: ActiveSelectionCopy,
		reason: SelectionExportCancel['reason'],
	): Promise<void> {
		await this.#host.requestControl(
			MessageKind.selectionExportCancel,
			{
				attachmentId: this.#bootstrap.attachmentId,
				exportSequence: state.exportSequence,
				reason,
			},
			MessageKind.selectionExportFinished,
			SelectionExportFinishedSchema,
		);
	}

	async #requestEncoded(
		kind: MessageKindValue,
		encoded: ReturnType<typeof encodeTextInput>,
		expectedKind: MessageKindValue,
	): Promise<void> {
		if (!encoded.ok) throw new Error(encoded.detail);
		await this.#host.request(kind, PayloadEncoding.rawBytes, encoded.value, expectedKind);
	}

	#postFailure(failure: ProjectedTerminalFailure): void {
		if (!this.#disposed) this.#port.postMessage({ failure, kind: 'failure' });
	}
}

function failureContext(
	command: ProjectedTerminalCommand,
): Pick<ProjectedTerminalFailure, 'operation' | 'requestSequence'> {
	switch (command.kind) {
		case 'selection-copy':
			return { operation: 'copy', requestSequence: command.payload.requestSequence };
		case 'link-open':
			return { operation: 'link', requestSequence: command.payload.requestSequence };
		case 'read-rows':
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
		case 'frame-applied':
		case 'need-snapshot':
		case 'set-projection-demand':
			return { operation: 'protocol' };
	}
}
