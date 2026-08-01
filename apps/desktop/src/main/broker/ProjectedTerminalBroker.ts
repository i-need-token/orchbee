import type { MessageEvent, MessagePortMain } from 'electron';

import {
	AttachmentAcknowledgedSchema,
	decodeControlJson,
	encodeFocusInput,
	encodeKeyInput,
	encodeMouseInput,
	encodePasteInput,
	encodeTextInput,
	MessageKind,
	PayloadEncoding,
	ProjectionBlockedSchema,
	TerminalExitedSchema,
	type DecodedProtocolEnvelope,
	type MessageKindValue,
} from '@orchbee/terminal-protocol';

import {
	DesktopRendererCommandSchema,
	type DesktopBrokerFailure,
	type DesktopRendererCommand,
	type ProjectedAttachmentBootstrap,
} from '../../shared';
import type { HostConnection } from '../host';
import { HostRequestError } from '../host';

const MAX_IN_FLIGHT_RENDERER_REQUESTS = 64;
const textEncoder = new TextEncoder();

export interface ProjectedTerminalBrokerMetrics {
	readonly commandsAccepted: number;
	readonly commandsRejected: number;
	readonly frameBytesForwarded: number;
	readonly framesForwarded: number;
}

export class ProjectedTerminalBroker {
	readonly #bootstrap: ProjectedAttachmentBootstrap;
	readonly #host: HostConnection;
	readonly #port: MessagePortMain;
	#commandsAccepted = 0;
	#commandsRejected = 0;
	#disposed = false;
	#frameBytesForwarded = 0;
	#framesForwarded = 0;
	#inFlightRequests = 0;
	#removeHostListener: (() => void) | undefined;

	constructor(
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
	) {
		this.#host = host;
		this.#port = port;
		this.#bootstrap = bootstrap;
	}

	get metrics(): ProjectedTerminalBrokerMetrics {
		return {
			commandsAccepted: this.#commandsAccepted,
			commandsRejected: this.#commandsRejected,
			frameBytesForwarded: this.#frameBytesForwarded,
			framesForwarded: this.#framesForwarded,
		};
	}

	start(): void {
		if (this.#disposed || this.#removeHostListener !== undefined) return;
		this.#removeHostListener = this.#host.onEnvelope(this.#handleHostEnvelope);
		this.#port.on('message', this.#handleRendererMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		this.#port.off('message', this.#handleRendererMessage);
		this.#port.off('close', this.#handlePortClose);
		this.#port.close();
		void this.#host
			.requestControl(
				MessageKind.detachTerminal,
				{ attachmentId: this.#bootstrap.attachmentId },
				MessageKind.detachTerminal,
				AttachmentAcknowledgedSchema,
			)
			.catch(() => undefined);
	}

	readonly #handlePortClose = (): void => this.dispose();

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const rawMessage: unknown = event.data;
		const parsed = DesktopRendererCommandSchema.safeParse(rawMessage);
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
				this.#postFailure(
					error instanceof HostRequestError
						? { code: error.code, message: error.message, retryable: error.retryable }
						: {
								code: 'host-request-failed',
								message: error instanceof Error ? error.message : 'Terminal Host request failed.',
								retryable: false,
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
				if (
					!event.ok ||
					event.value.identity.creationId !== this.#bootstrap.creationId ||
					event.value.identity.hostEpoch !== this.#bootstrap.hostEpoch ||
					event.value.identity.terminalId !== this.#bootstrap.terminalId
				) {
					this.#postFailure({
						code: 'invalid-terminal-exit',
						message: event.ok
							? 'Terminal exit identity is outside the scoped attachment.'
							: event.detail,
						retryable: false,
					});
					return;
				}
				this.#port.postMessage({ event: event.value, kind: 'terminal-exited' });
				return;
			}
			case MessageKind.projectionBlocked: {
				const blocked = decodeControlJson(envelope.payload, ProjectionBlockedSchema);
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

	async #dispatch(command: DesktopRendererCommand): Promise<void> {
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
			case 'resize-terminal':
				await this.#host.request(
					MessageKind.resizeTerminal,
					PayloadEncoding.controlJson,
					textEncoder.encode(JSON.stringify(command.payload)),
					MessageKind.resizeAccepted,
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
			case 'text-input':
				return this.#requestEncoded(
					MessageKind.textInput,
					encodeTextInput(command.payload),
					MessageKind.inputAccepted,
				);
		}
	}

	async #requestEncoded(
		kind: MessageKindValue,
		encoded: ReturnType<typeof encodeTextInput>,
		expectedKind: MessageKindValue,
	): Promise<void> {
		if (!encoded.ok) throw new Error(encoded.detail);
		await this.#host.request(kind, PayloadEncoding.rawBytes, encoded.value, expectedKind);
	}

	#postFailure(failure: DesktopBrokerFailure): void {
		if (!this.#disposed) this.#port.postMessage({ failure, kind: 'failure' });
	}
}
