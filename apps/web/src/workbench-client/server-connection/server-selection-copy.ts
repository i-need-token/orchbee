/**
 * Assembles one Host selection export into the bytes a Client's clipboard effect receives. Chunks
 * arrive through Poll and the Host sends the next only after this one is acknowledged. Closing the
 * viewer's lane releases an export in progress, so retiring cancels nothing.
 */
import {
	type CanonicalUint64Text,
	type DecodedProtocolEnvelope,
	MessageKind,
	PROTOCOL_LIMITS,
	PayloadEncoding,
	type SelectionExportChunk,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	TypedFailureSchema,
	decodeControlJson,
	decodeProtocolEnvelope,
	decodeSelectionExportChunk,
} from '@orchbee/terminal-protocol';
import type { ProjectedTerminalEvent } from '@orchbee/workbench-client-protocol';

import type { ClientServerResponse, ClientServerViewerAction } from './client-server-carrier';

type SelectionAnswer =
	| { readonly kind: 'accepted' }
	| { readonly envelope: DecodedProtocolEnvelope; readonly kind: 'record' }
	| {
			readonly code: string;
			readonly kind: 'refused';
			readonly message: string;
			readonly retryable: boolean;
	  };

/** A selection action answers with a Host record, a Host failure inside one, or a Server refusal. */
export function readSelectionAnswer(response: ClientServerResponse): SelectionAnswer {
	if (response.kind === 'terminal.viewer.record') {
		const decoded = decodeProtocolEnvelope(response.hostRecord);
		if (!decoded.ok) {
			return {
				code: 'invalid-host-record',
				kind: 'refused',
				message: decoded.detail,
				retryable: false,
			};
		}
		if (decoded.value.kind !== MessageKind.typedFailure) {
			return { envelope: decoded.value, kind: 'record' };
		}
		const failure = decodeControlJson(decoded.value.payload, TypedFailureSchema);
		return failure.ok
			? { ...failure.value, kind: 'refused' }
			: { code: 'invalid-host-record', kind: 'refused', message: failure.detail, retryable: false };
	}
	if (response.kind === 'terminal.viewer.result') {
		const { outcome } = response;
		if (outcome.kind === 'feedback-accepted') return { kind: 'accepted' };
		if ('reason' in outcome) {
			return {
				code: outcome.reason,
				kind: 'refused',
				message: `The Server refused: ${outcome.reason}.`,
				retryable: outcome.kind === 'unavailable',
			};
		}
	}
	return {
		code: 'invalid-selection-response',
		kind: 'refused',
		message: 'The Server answered a selection action with another message.',
		retryable: false,
	};
}

interface ServerSelectionCopyPort {
	readonly attachmentId: string;
	exchange(action: ClientServerViewerAction): Promise<ClientServerResponse>;
	onChunkAcknowledged(): void;
	post(event: ProjectedTerminalEvent): void;
}

interface ActiveExport {
	buffer: Uint8Array<ArrayBuffer> | null;
	/** Poll and the export's own answer are separate streams, so a chunk can win the race. */
	earlyChunk: SelectionExportChunk | null;
	readonly exportSequence: bigint;
	finalReceived: boolean;
	nextChunkSequence: number;
	offset: number;
	readonly requestSequence: CanonicalUint64Text;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'The Server exchange failed.';
}

export class ServerSelectionCopy {
	readonly #port: ServerSelectionCopyPort;
	#active: ActiveExport | null = null;
	#nextExportSequence = 1n;

	constructor(port: ServerSelectionCopyPort) {
		this.#port = port;
	}

	/** Never rejects: it runs as one serial step, and every outcome reaches the renderer as an event. */
	async start(requestSequence: CanonicalUint64Text): Promise<void> {
		if (this.#active !== null) {
			this.#postFailure({
				code: 'copy-in-progress',
				message: 'A terminal selection copy is already in progress.',
				requestSequence,
				retryable: true,
			});
			return;
		}
		const active: ActiveExport = {
			buffer: null,
			earlyChunk: null,
			exportSequence: this.#nextExportSequence++,
			finalReceived: false,
			nextChunkSequence: 0,
			offset: 0,
			requestSequence,
		};
		this.#active = active;
		let response: ClientServerResponse;
		try {
			response = await this.#port.exchange({
				attachmentId: this.#port.attachmentId,
				exportSequence: active.exportSequence.toString(),
				format: 'plain',
				kind: 'selection-export',
			});
		} catch (error) {
			this.#fail(active, 'outcome-unknown', errorMessage(error), false);
			return;
		}
		if (this.#active !== active) return;
		const answer = readSelectionAnswer(response);
		if (answer.kind === 'refused') {
			this.#fail(active, answer.code, answer.message, answer.retryable);
			return;
		}
		const started =
			answer.kind === 'record' && answer.envelope.kind === MessageKind.selectionExportStarted
				? decodeControlJson(answer.envelope.payload, SelectionExportStartedSchema)
				: null;
		if (
			started?.ok !== true ||
			started.value.attachmentId !== this.#port.attachmentId ||
			BigInt(started.value.exportSequence) !== active.exportSequence
		) {
			this.#fail(
				active,
				'invalid-copy-export',
				'The Host started an export outside this copy.',
				false,
			);
			return;
		}
		active.buffer = new Uint8Array(started.value.byteLength);
		const { earlyChunk } = active;
		active.earlyChunk = null;
		if (earlyChunk !== null) this.#apply(active, active.buffer, earlyChunk);
	}

	receiveChunk(envelope: DecodedProtocolEnvelope): void {
		const decoded =
			envelope.payloadEncoding === PayloadEncoding.rawBytes
				? decodeSelectionExportChunk(envelope.payload)
				: null;
		if (decoded?.ok !== true || decoded.value.attachmentId !== this.#port.attachmentId) {
			this.#rejectRecord('invalid-copy-chunk', 'The Host sent an invalid selection export chunk.');
			return;
		}
		const chunk = decoded.value;
		if (this.#isAbandoned(chunk.exportSequence)) return;
		const active = this.#active;
		if (active?.exportSequence !== chunk.exportSequence) {
			this.#rejectRecord('invalid-copy-chunk', 'The Host sent a chunk of an unknown export.');
			return;
		}
		if (active.buffer !== null) {
			this.#apply(active, active.buffer, chunk);
		} else if (active.earlyChunk === null) {
			active.earlyChunk = chunk;
		} else {
			this.#rejectRecord('invalid-copy-chunk', 'The Host sent a chunk before an acknowledgement.');
		}
	}

	receiveFinished(envelope: DecodedProtocolEnvelope): void {
		const decoded = decodeControlJson(envelope.payload, SelectionExportFinishedSchema);
		if (!decoded.ok || decoded.value.attachmentId !== this.#port.attachmentId) {
			this.#rejectRecord('invalid-copy-finish', 'The Host finished an export outside this viewer.');
			return;
		}
		const finished = decoded.value;
		const exportSequence = BigInt(finished.exportSequence);
		if (this.#isAbandoned(exportSequence)) return;
		const active = this.#active;
		if (active?.exportSequence !== exportSequence) {
			this.#rejectRecord('invalid-copy-finish', 'The Host finished an unknown export.');
			return;
		}
		const { buffer } = active;
		if (
			finished.status !== 'completed' ||
			buffer === null ||
			!active.finalReceived ||
			active.offset !== buffer.byteLength ||
			finished.byteLength !== buffer.byteLength
		) {
			this.#fail(
				active,
				'copy-cancelled',
				'The terminal selection export did not complete.',
				finished.status === 'cancelled',
			);
			return;
		}
		this.#active = null;
		this.#port.post({
			content: buffer.buffer,
			kind: 'selection-copy-content',
			requestSequence: active.requestSequence,
		});
	}

	retire(): void {
		this.#active = null;
	}

	/** A failed or lost export can still deliver what was already on its way here. */
	#isAbandoned(exportSequence: bigint): boolean {
		return (
			exportSequence < this.#nextExportSequence && exportSequence !== this.#active?.exportSequence
		);
	}

	#apply(active: ActiveExport, buffer: Uint8Array, chunk: SelectionExportChunk): void {
		const end = active.offset + chunk.bytes.byteLength;
		if (
			active.finalReceived ||
			chunk.chunkSequence !== active.nextChunkSequence ||
			chunk.bytes.byteLength > PROTOCOL_LIMITS.maxSelectionExportChunkBytes ||
			end > buffer.byteLength ||
			(chunk.final && end !== buffer.byteLength)
		) {
			this.#failAndCancel(
				active,
				'invalid-copy-chunk',
				'The Host sent a non-canonical selection export chunk.',
				false,
			);
			return;
		}
		buffer.set(chunk.bytes, active.offset);
		active.offset = end;
		active.nextChunkSequence += 1;
		active.finalReceived = chunk.final;
		void this.#acknowledge(active, chunk.chunkSequence);
	}

	async #acknowledge(active: ActiveExport, chunkSequence: number): Promise<void> {
		let response: ClientServerResponse;
		try {
			response = await this.#port.exchange({
				attachmentId: this.#port.attachmentId,
				chunkSequence,
				exportSequence: active.exportSequence.toString(),
				kind: 'selection-export-chunk-applied',
			});
		} catch (error) {
			this.#fail(active, 'outcome-unknown', errorMessage(error), false);
			return;
		}
		const answer = readSelectionAnswer(response);
		if (answer.kind === 'accepted') {
			this.#port.onChunkAcknowledged();
		} else if (answer.kind === 'refused') {
			this.#failAndCancel(active, answer.code, answer.message, answer.retryable);
		} else {
			this.#failAndCancel(
				active,
				'invalid-copy-chunk',
				'The Host answered a chunk acknowledgement with another record.',
				false,
			);
		}
	}

	#rejectRecord(code: string, message: string): void {
		if (this.#active === null) {
			this.#postFailure({ code, message, retryable: false });
		} else {
			this.#failAndCancel(this.#active, code, message, false);
		}
	}

	/** The Host keeps an export waiting for an acknowledgement that this failure will never send. */
	#failAndCancel(active: ActiveExport, code: string, message: string, retryable: boolean): void {
		if (this.#active !== active) return;
		this.#fail(active, code, message, retryable);
		this.#port
			.exchange({
				attachmentId: this.#port.attachmentId,
				exportSequence: active.exportSequence.toString(),
				kind: 'selection-export-cancel',
				reason: 'replaced',
			})
			.catch(() => undefined);
	}

	#fail(active: ActiveExport, code: string, message: string, retryable: boolean): void {
		if (this.#active !== active) return;
		this.#active = null;
		this.#postFailure({ code, message, requestSequence: active.requestSequence, retryable });
	}

	#postFailure(failure: {
		readonly code: string;
		readonly message: string;
		readonly requestSequence?: CanonicalUint64Text;
		readonly retryable: boolean;
	}): void {
		this.#port.post({ failure: { ...failure, operation: 'copy' }, kind: 'failure' });
	}
}
