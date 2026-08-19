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
	FrameAppliedSchema,
	HistoryChangedSchema,
	HistoryDemandAcceptedSchema,
	HistoryObservedSchema,
	MessageKind,
	NeedSnapshotSchema,
	PayloadEncoding,
	ProjectionBlockedSchema,
	ProjectionDemandAcceptedSchema,
	PROTOCOL_LIMITS,
	ResizeAcceptedSchema,
	ResizeLeaseChangedSchema,
	RowsAppliedSchema,
	SelectionChangedSchema,
	SelectionExportChunkAppliedSchema,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	TerminalExitedSchema,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	TerminalFailedSchema,
	TerminalControlAcceptedSchema,
	TerminalThemeResultSchema,
	type DecodedProtocolEnvelope,
	type MessageKindValue,
	type SelectionExportChunk,
	type SelectionExportCancel,
	type TerminalClipboardWriteRequest,
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import { HostRequestError, readTerminalInventory, type HostConnection } from '../host';

const MAX_IN_FLIGHT_RENDERER_REQUESTS = 64;
const fatalTextDecoder = new TextDecoder('utf-8', { fatal: true });

function copyExactArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}

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

export interface ProjectedTerminalBrokerLifecycle {
	readonly onCleanupRequested: (error: unknown) => void;
	readonly onResizeLeaseChanged?: (ownsResize: boolean) => void;
	readonly onThemeChanged?: (theme: TerminalThemeState) => void;
}

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
	readonly #onCleanupRequested: ((error: unknown) => void) | undefined;
	readonly #onResizeLeaseChanged: ((ownsResize: boolean) => void) | undefined;
	readonly #onThemeChanged: ((theme: TerminalThemeState) => void) | undefined;
	readonly #port: MessagePortMain;
	#activeSelectionCopy: ActiveSelectionCopy | undefined;
	#clipboardCommits = 0;
	#commandsAccepted = 0;
	#commandsRejected = 0;
	#cleanupRequested = false;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	#externalTargetsOpened = 0;
	#frameBytesForwarded = 0;
	#framesForwarded = 0;
	#inFlightRequests = 0;
	#lastSelectionCopyMilliseconds: number | null = null;
	#lastSelectionExportStartMilliseconds: number | null = null;
	#latestTerminalOutcomeRevision: bigint | undefined;
	#nextExportSequence = 1n;
	#pendingClipboardWriteRequest: TerminalClipboardWriteRequest | undefined;
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
		lifecycle?: ProjectedTerminalBrokerLifecycle,
	) {
		this.#host = host;
		this.#onCleanupRequested = lifecycle?.onCleanupRequested;
		this.#onResizeLeaseChanged = lifecycle?.onResizeLeaseChanged;
		this.#onThemeChanged = lifecycle?.onThemeChanged;
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
		const removeClosedListener = this.#host.onClosed(this.#handleHostClosed);
		const removeControlListener = this.#host.onEnvelope(this.#handleHostEnvelope);
		this.#removeHostListener = () => {
			removeFrameListener();
			removeClosedListener();
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
		this.#pendingClipboardWriteRequest = undefined;
		this.#disposed = true;
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		this.#port.off('message', this.#handleRendererMessage);
		this.#port.off('close', this.#handlePortClose);
		this.#port.close();
		this.#disposePromise = this.#detach(activeCopy).catch((error: unknown) => {
			// Main-side listeners stay revoked after a failed detach; retrying the
			// same AttachmentId is the only safe way to converge Host cleanup.
			this.#disposePromise = undefined;
			throw error;
		});
		return this.#disposePromise;
	}

	readonly #handlePortClose = (): void => {
		this.#requestCleanup(
			new BrokerOperationError(
				'renderer-port-closed',
				'Renderer attachment port closed; the Host attachment must be detached.',
				true,
			),
		);
	};

	readonly #handleHostClosed = (reason: Error): void => {
		if (this.#disposed || this.#cleanupRequested) return;
		this.#postFailure({
			code: 'host-connection-unavailable',
			message: 'The Terminal Host connection is unavailable.',
			operation: 'protocol',
			retryable: true,
		});
		this.#requestCleanup(reason);
	};

	#requestCleanup(error: unknown): void {
		if (this.#cleanupRequested) return;
		this.#cleanupRequested = true;
		if (this.#onCleanupRequested !== undefined) {
			try {
				this.#onCleanupRequested(error);
			} catch {
				void this.dispose().catch(() => undefined);
			}
			return;
		}
		void this.dispose().catch(() => undefined);
	}

	#postMessage(message: unknown, transfer?: readonly MessagePortMain[]): boolean {
		if (this.#disposed) return false;
		try {
			if (transfer === undefined) this.#port.postMessage(message);
			else this.#port.postMessage(message, [...transfer]);
			return true;
		} catch (error: unknown) {
			this.#requestCleanup(error);
			return false;
		}
	}

	async #detach(activeCopy: ActiveSelectionCopy | undefined): Promise<void> {
		if (activeCopy !== undefined) {
			await this.#requestSelectionExportCancel(activeCopy, 'disposed').catch(() => undefined);
		}
		const acknowledgement = await this.#host.requestControl(
			MessageKind.detachTerminal,
			{ attachmentId: this.#bootstrap.attachmentId },
			AttachmentAcknowledgedSchema,
		);
		if (acknowledgement.attachmentId !== this.#bootstrap.attachmentId) {
			throw new BrokerOperationError(
				'attachment-ack-mismatch',
				'Terminal Host acknowledged a different attachment during detach.',
				true,
			);
		}
	}

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed || this.#cleanupRequested) return;
		const rawMessage: unknown = event.data;
		const parsed = ProjectedTerminalCommandSchema.safeParse(rawMessage);
		if (!parsed.success || parsed.data.payload.attachmentId !== this.#bootstrap.attachmentId) {
			this.#commandsRejected += 1;
			this.#postFailure({
				code: 'renderer-capability-rejected',
				message: 'Renderer command is invalid or outside its attachment capability.',
				...(parsed.success ? failureContext(parsed.data) : { operation: 'protocol' as const }),
				retryable: false,
			});
			return;
		}
		if (this.#inFlightRequests >= MAX_IN_FLIGHT_RENDERER_REQUESTS) {
			if (
				parsed.data.kind === 'clipboard-write-decision' &&
				this.#matchesPendingClipboardWriteRequest(parsed.data.payload)
			) {
				this.#pendingClipboardWriteRequest = undefined;
			}
			this.#commandsRejected += 1;
			if (isThemeCommand(parsed.data)) {
				this.#postThemeFailure(parsed.data.payload.operationId, {
					code: 'renderer-backpressure',
					outcome: 'known-unchanged',
					retryable: true,
				});
			} else {
				this.#postFailure({
					code: 'renderer-backpressure',
					message: 'Renderer command exceeded the bounded Main request window.',
					...failureContext(parsed.data),
					retryable: true,
				});
			}
			this.#requestCleanup(
				new BrokerOperationError(
					'renderer-backpressure',
					'Renderer command exceeded the bounded Main request window.',
					true,
				),
			);
			return;
		}

		this.#commandsAccepted += 1;
		this.#inFlightRequests += 1;
		void this.#dispatch(parsed.data)
			.catch((error: unknown) => {
				if (isThemeCommand(parsed.data)) {
					this.#postThemeFailure(parsed.data.payload.operationId, themeFailureFromError(error));
					return;
				}
				const context = failureContext(parsed.data);
				this.#postFailure(
					error instanceof HostRequestError || error instanceof BrokerOperationError
						? {
								code: error.code,
								...(error instanceof HostRequestError && error.correlationId !== undefined
									? { correlationId: error.correlationId }
									: {}),
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
				if (error instanceof HostRequestError && error.code === 'host-transport-backpressure') {
					this.#requestCleanup(error);
				}
			})
			.finally(() => {
				this.#inFlightRequests -= 1;
			});
	};

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (this.#disposed || this.#cleanupRequested) return;
		switch (envelope.kind) {
			case MessageKind.resizeLeaseChanged: {
				const event = decodeControlJson(envelope.payload, ResizeLeaseChangedSchema);
				if (
					!event.ok ||
					event.value.attachmentId !== this.#bootstrap.attachmentId ||
					event.value.terminalId !== this.#bootstrap.terminalId
				) {
					this.#postFailure({
						code: 'invalid-resize-lease-event',
						message: event.ok
							? 'Terminal Host resize-lease event is outside the scoped attachment.'
							: event.detail,
						operation: 'resize',
						retryable: false,
					});
					return;
				}
				try {
					this.#onResizeLeaseChanged?.(event.value.resizeLease);
				} catch (error: unknown) {
					this.#requestCleanup(error);
					return;
				}
				this.#postMessage({ event: event.value, kind: 'resize-lease-changed' });
				return;
			}
			case MessageKind.terminalBell: {
				const event = decodeControlJson(envelope.payload, TerminalBellSchema);
				if (!event.ok || event.value.attachmentId !== this.#bootstrap.attachmentId) {
					this.#postFailure({
						code: 'invalid-bell-event',
						message: event.ok
							? 'Terminal Host bell event is outside the scoped attachment.'
							: event.detail,
						operation: 'protocol',
						retryable: false,
					});
					return;
				}
				this.#postMessage({ event: event.value, kind: 'bell' });
				return;
			}
			case MessageKind.terminalClipboardWriteRequest: {
				const event = decodeControlJson(envelope.payload, TerminalClipboardWriteRequestSchema);
				if (!event.ok || event.value.attachmentId !== this.#bootstrap.attachmentId) {
					this.#postFailure({
						code: 'invalid-clipboard-write-request',
						message: event.ok
							? 'Terminal Host clipboard request is outside the scoped attachment.'
							: event.detail,
						operation: 'protocol',
						retryable: false,
					});
					return;
				}
				if (this.#pendingClipboardWriteRequest !== undefined) return;
				if (this.#postMessage({ event: event.value, kind: 'clipboard-write-request' })) {
					this.#pendingClipboardWriteRequest = event.value;
				}
				return;
			}
			case MessageKind.historyChanged: {
				const event = decodeControlJson(envelope.payload, HistoryChangedSchema);
				if (!event.ok || event.value.attachmentId !== this.#bootstrap.attachmentId) {
					this.#postFailure({
						code: 'invalid-history-event',
						message: event.ok
							? 'Terminal Host history event is outside the scoped attachment.'
							: event.detail,
						operation: 'history',
						retryable: false,
					});
					return;
				}
				this.#postMessage({ event: event.value, kind: 'history-changed' });
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
						operation: 'selection',
						retryable: false,
					});
					return;
				}
				this.#postMessage({ event: event.value, kind: 'selection-changed' });
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
						operation: 'protocol',
						retryable: false,
					});
					return;
				}
				this.#framesForwarded += 1;
				this.#frameBytesForwarded += envelope.payload.byteLength;
				this.#postMessage({ frame: copyExactArrayBuffer(envelope.payload), kind: 'frame' });
				return;
			}
			case MessageKind.terminalExited: {
				const event = decodeControlJson(envelope.payload, TerminalExitedSchema);
				if (!event.ok) {
					this.#postFailure({
						code: 'invalid-terminal-exit',
						message: event.detail,
						operation: 'protocol',
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
				const revision = BigInt(event.value.finalRevision);
				if (
					this.#latestTerminalOutcomeRevision !== undefined &&
					revision <= this.#latestTerminalOutcomeRevision
				) {
					return;
				}
				if (this.#postMessage({ event: event.value, kind: 'terminal-exited' })) {
					this.#latestTerminalOutcomeRevision = revision;
				}
				return;
			}
			case MessageKind.terminalFailed: {
				const event = decodeControlJson(envelope.payload, TerminalFailedSchema);
				if (!event.ok) {
					this.#postFailure({
						code: 'invalid-terminal-failure',
						message: event.detail,
						operation: 'protocol',
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
				const revision = BigInt(event.value.finalRevision);
				if (
					this.#latestTerminalOutcomeRevision !== undefined &&
					revision <= this.#latestTerminalOutcomeRevision
				) {
					return;
				}
				if (this.#postMessage({ event: event.value, kind: 'terminal-failed' })) {
					this.#latestTerminalOutcomeRevision = revision;
				}
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
					operation: 'protocol',
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
			case 'apply-terminal-theme': {
				const result = await this.#host.requestControl(
					MessageKind.applyTerminalTheme,
					{
						expectedThemeRevision: command.payload.expectedThemeRevision,
						operationId: command.payload.operationId,
						terminal: command.payload.terminal,
						terminalId: command.payload.terminalId,
					},
					TerminalThemeResultSchema,
				);
				if (
					result.operationId !== command.payload.operationId ||
					result.terminalId !== this.#bootstrap.terminalId
				) {
					throw new BrokerOperationError(
						'invalid-terminal-theme-response',
						'Terminal Host returned a theme result outside the scoped operation.',
					);
				}
				this.#onThemeChanged?.(result.current);
				this.#postMessage({ event: result, kind: 'terminal-theme-result' });
				return;
			}
			case 'clear-terminal':
				return this.#requestTerminalControl(MessageKind.clearTerminal, command.payload, 'clear');
			case 'clipboard-write-decision': {
				if (!this.#matchesPendingClipboardWriteRequest(command.payload)) {
					throw new BrokerOperationError(
						'clipboard-decision-mismatch',
						'Terminal clipboard decision does not match the pending Host request.',
					);
				}
				this.#pendingClipboardWriteRequest = undefined;
				if (command.payload.approved && this.#effectTarget === 'desktop') {
					await this.#dependencies.writeClipboardText(command.payload.text);
					this.#clipboardCommits += 1;
				}
				return;
			}
			case 'focus-input':
				return this.#requestInput(MessageKind.focusInput, command.payload);
			case 'frame-applied':
				await this.#host.requestControl(
					MessageKind.frameApplied,
					command.payload,
					FrameAppliedSchema,
				);
				return;
			case 'key-input':
				return this.#requestInput(MessageKind.keyInput, command.payload);
			case 'mouse-input':
				return this.#requestInput(MessageKind.mouseInput, command.payload);
			case 'need-snapshot':
				await this.#host.requestControl(
					MessageKind.needSnapshot,
					command.payload,
					NeedSnapshotSchema,
				);
				return;
			case 'paste-input':
				return this.#requestInput(MessageKind.pasteInput, command.payload);
			case 'reconcile-terminal-theme': {
				const inventory = await readTerminalInventory(this.#host);
				const summary = inventory.find(
					(candidate) =>
						candidate.identity.creationId === this.#bootstrap.creationId &&
						candidate.identity.hostEpoch === this.#bootstrap.hostEpoch &&
						candidate.identity.terminalId === this.#bootstrap.terminalId,
				);
				if (summary === undefined) {
					throw new BrokerOperationError(
						'terminal-theme-reconciliation-unavailable',
						'Terminal Host inventory no longer contains the scoped terminal.',
						true,
					);
				}
				this.#onThemeChanged?.(summary.theme);
				this.#postMessage({
					current: summary.theme,
					kind: 'terminal-theme-reconciled',
					operationId: command.payload.operationId,
					terminalId: summary.identity.terminalId,
				});
				return;
			}
			case 'read-rows': {
				const response = await this.#host.requestControlEnvelope(
					MessageKind.readRows,
					command.payload,
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
				this.#rowWindowsForwarded += 1;
				this.#rowWindowBytesForwarded += response.payload.byteLength;
				this.#postMessage({ kind: 'rows', rows: copyExactArrayBuffer(response.payload) });
				return;
			}
			case 'resize-terminal':
				await this.#host.requestControl(
					MessageKind.resizeTerminal,
					command.payload,
					ResizeAcceptedSchema,
				);
				return;
			case 'reset-terminal':
				return this.#requestTerminalControl(MessageKind.resetTerminal, command.payload, 'reset');
			case 'rows-applied':
				await this.#host.requestControl(
					MessageKind.rowsApplied,
					command.payload,
					RowsAppliedSchema,
				);
				return;
			case 'history-observed':
				await this.#host.requestControl(
					MessageKind.historyObserved,
					command.payload,
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
					SelectionChangedSchema,
				);
				if (selection.attachmentId !== this.#bootstrap.attachmentId) {
					throw new BrokerOperationError(
						'invalid-selection-response',
						'Terminal Host selection response is outside the scoped attachment.',
					);
				}
				this.#postMessage({ event: selection, kind: 'selection-changed' });
				return;
			}
			case 'selection-copy':
				await this.#startSelectionCopy(command.payload.requestSequence);
				return;
			case 'set-history-demand':
				await this.#host.requestControl(
					MessageKind.setHistoryDemand,
					command.payload,
					HistoryDemandAcceptedSchema,
				);
				return;
			case 'set-projection-demand':
				await this.#host.requestControl(
					MessageKind.setProjectionDemand,
					command.payload,
					ProjectionDemandAcceptedSchema,
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
				this.#postMessage({
					kind: 'link-opened',
					requestSequence: command.payload.requestSequence,
				});
				return;
			case 'text-input':
				return this.#requestInput(MessageKind.textInput, command.payload);
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
			state !== undefined &&
			decoded.value.status === 'cancelled' &&
			decoded.value.attachmentId === this.#bootstrap.attachmentId &&
			state.byteLength === undefined &&
			state.buffer === undefined &&
			BigInt(decoded.value.exportSequence) < BigInt(state.exportSequence)
		) {
			return;
		}
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
			this.#postMessage({
				content: content.buffer,
				kind: 'selection-copy-content',
				requestSequence: state.requestSequence,
			});
		}
		if (this.#activeSelectionCopy !== state) return;
		this.#lastSelectionCopyMilliseconds = performance.now() - state.startedAtMilliseconds;
		this.#activeSelectionCopy = undefined;
		if (this.#effectTarget === 'desktop') {
			this.#postMessage({
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

	#matchesPendingClipboardWriteRequest(request: TerminalClipboardWriteRequest): boolean {
		const pending = this.#pendingClipboardWriteRequest;
		return (
			pending !== undefined &&
			pending.attachmentId === request.attachmentId &&
			pending.sequence === request.sequence &&
			pending.text === request.text
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
			SelectionExportFinishedSchema,
		);
	}

	async #requestInput(kind: MessageKindValue, value: unknown): Promise<void> {
		await this.#host.requestInput(kind, value);
	}

	async #requestTerminalControl(
		kind: typeof MessageKind.clearTerminal | typeof MessageKind.resetTerminal,
		value: unknown,
		operation: 'clear' | 'reset',
	): Promise<void> {
		const accepted = await this.#host.requestControl(kind, value, TerminalControlAcceptedSchema);
		if (
			accepted.attachmentId !== this.#bootstrap.attachmentId ||
			accepted.terminalId !== this.#bootstrap.terminalId ||
			accepted.operation !== operation
		) {
			throw new BrokerOperationError(
				'invalid-terminal-control-response',
				'Terminal Host acknowledged a different terminal control operation.',
			);
		}
	}

	#postFailure(failure: ProjectedTerminalFailure): void {
		this.#postMessage({ failure, kind: 'failure' });
	}

	#postThemeFailure(
		operationId: Extract<
			ProjectedTerminalCommand,
			{ readonly kind: 'apply-terminal-theme' | 'reconcile-terminal-theme' }
		>['payload']['operationId'],
		failure: {
			readonly code: string;
			readonly outcome: 'known-unchanged' | 'unknown';
			readonly retryable: boolean;
		},
	): void {
		this.#postMessage({ failure, kind: 'terminal-theme-failed', operationId });
	}
}

function failureContext(
	command: ProjectedTerminalCommand,
): Pick<ProjectedTerminalFailure, 'operation' | 'requestSequence'> {
	switch (command.kind) {
		case 'apply-terminal-theme':
		case 'reconcile-terminal-theme':
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
		case 'frame-applied':
		case 'need-snapshot':
		case 'set-projection-demand':
			return { operation: 'protocol' };
	}
}

function isThemeCommand(
	command: ProjectedTerminalCommand,
): command is Extract<
	ProjectedTerminalCommand,
	{ readonly kind: 'apply-terminal-theme' | 'reconcile-terminal-theme' }
> {
	return command.kind === 'apply-terminal-theme' || command.kind === 'reconcile-terminal-theme';
}

function themeFailureFromError(error: unknown): {
	readonly code: string;
	readonly outcome: 'known-unchanged' | 'unknown';
	readonly retryable: boolean;
} {
	if (error instanceof HostRequestError) {
		return {
			code: error.code,
			outcome: error.code === 'host-request-outcome-unknown' ? 'unknown' : 'known-unchanged',
			retryable: error.retryable,
		};
	}
	if (error instanceof BrokerOperationError) {
		return { code: error.code, outcome: 'unknown', retryable: error.retryable };
	}
	return { code: 'host-request-failed', outcome: 'unknown', retryable: false };
}
