import { z } from 'zod';

import { BinaryReader, BinaryWriter, checkedAdd } from './binary';
import {
	AttachmentIdSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseAttachmentId,
} from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

const SELECTION_EXPORT_CHUNK_HEADER_BYTES = 32;
const MAX_UINT32 = 0xffff_ffff;
const MAX_UINT64 = 0xffff_ffff_ffff_ffffn;

const SelectionExportChunkSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	bytes: z
		.instanceof(Uint8Array)
		.refine(
			(value) => value.byteLength <= PROTOCOL_LIMITS.maxSelectionExportChunkBytes,
			'Selection export chunk exceeds its limit',
		),
	chunkSequence: z.number().int().min(0).max(MAX_UINT32),
	exportSequence: z.bigint().min(1n).max(MAX_UINT64),
	final: z.boolean(),
});

export type SelectionExportChunk = z.infer<typeof SelectionExportChunkSchema>;

interface DecodedSelectionExportChunkRoute {
	readonly attachmentId: z.infer<typeof AttachmentIdSchema>;
	readonly chunkSequence: number;
	readonly exportSequence: bigint;
	readonly final: boolean;
}

export function encodeSelectionExportChunk(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = SelectionExportChunkSchema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Selection export chunk validation failed: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}
	const totalLength = checkedAdd(SELECTION_EXPORT_CHUNK_HEADER_BYTES, parsed.data.bytes.byteLength);
	if (!totalLength.ok) return totalLength;
	if (totalLength.value > PROTOCOL_LIMITS.maxRawPayloadBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Selection export chunk is too large');
	}
	const writer = new BinaryWriter(totalLength.value);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(parsed.data.exportSequence);
	writer.writeUint32(parsed.data.chunkSequence);
	writer.writeUint8(parsed.data.final ? 1 : 0);
	writer.writeBytes(new Uint8Array(3));
	writer.writeBytes(parsed.data.bytes);
	return protocolSuccess(writer.bytes());
}

export function decodeSelectionExportChunk(
	bytes: Uint8Array,
): ProtocolResult<SelectionExportChunk> {
	const route = decodeSelectionExportChunkRoute(bytes);
	if (!route.ok) return route;
	const payload = bytes.subarray(SELECTION_EXPORT_CHUNK_HEADER_BYTES);
	const chunkBytes = new Uint8Array(payload.byteLength);
	chunkBytes.set(payload);
	return protocolSuccess({ ...route.value, bytes: chunkBytes });
}

/** @internal Exported for this package's selection tests; production decodes the route inside `decodeSelectionExportChunk`. */
export function decodeSelectionExportChunkRoute(
	bytes: Uint8Array,
): ProtocolResult<DecodedSelectionExportChunkRoute> {
	if (bytes.byteLength < SELECTION_EXPORT_CHUNK_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.truncated,
			`Selection export chunk requires ${SELECTION_EXPORT_CHUNK_HEADER_BYTES} header bytes`,
		);
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxRawPayloadBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Selection export chunk is too large');
	}
	const reader = new BinaryReader(bytes);
	const attachmentBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const exportSequence = reader.readUint64();
	const chunkSequence = reader.readUint32();
	const flags = reader.readUint8();
	const reserved = reader.readBytes(3);
	if (reader.failure) return reader.failure;
	if ((flags & ~1) !== 0 || reserved.some((value) => value !== 0)) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Selection export chunk contains a non-zero reserved or required flag',
		);
	}
	if (exportSequence === 0n) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Export sequence must be positive');
	}
	const attachmentText = identifierText(attachmentBytes);
	if (!attachmentText.ok) return attachmentText;
	const attachmentId = parseAttachmentId(attachmentText.value);
	if (!attachmentId.ok) return attachmentId;
	return protocolSuccess({
		attachmentId: attachmentId.value,
		chunkSequence,
		exportSequence,
		final: (flags & 1) !== 0,
	});
}
