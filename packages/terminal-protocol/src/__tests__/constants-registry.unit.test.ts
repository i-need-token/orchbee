import { readFile } from 'node:fs/promises';
import { expect, it } from 'vitest';
import { z } from 'zod';

import {
	encodeProtocolEnvelope,
	TRANSPORT_ENVELOPE_HEADER_BYTES,
	TRANSPORT_ENVELOPE_MAGIC,
} from '../envelope';
import { encodeTerminalFrame, TerminalFrameKind, TerminalFrameSchema } from '../frame';
import { PROTOCOL_LIMITS } from '../limits';
import { MESSAGE_CONTRACTS, MessageKind, PayloadEncoding } from '../message-contract';
import { encodeTerminalProjectionBuffer } from '../projection-buffer';
import type { ProtocolResult } from '../protocol-result';
import { encodeTerminalRowWindow, TerminalRowWindowDetail } from '../row-window';
import { encodeTerminalGraphics, TerminalGraphicsUpdateKind } from '../terminal-graphics';
import { CURRENT_PROTOCOL_VERSION, type ProtocolVersion } from '../version';

const canonicalUint64Schema = z.string().transform((value) => BigInt(value));

// The JSON fixture spells 64-bit counters as decimal text.
const CanonicalFrameFixtureSchema = z.object({
	frame: z.looseObject({
		generation: canonicalUint64Schema,
		revision: canonicalUint64Schema,
		scrollbackRows: canonicalUint64Schema,
		sequence: canonicalUint64Schema,
		themeRevision: canonicalUint64Schema,
	}),
});

function format(magic: Uint8Array, version: ProtocolVersion, headerBytes: number) {
	return { magic: new TextDecoder().decode(magic), version, header_bytes: headerBytes };
}

function valueOf<T>(result: ProtocolResult<T>) {
	if (!result.ok) throw new Error(result.detail);
	return result.value;
}

function encodedFormat(
	result: ProtocolResult<Uint8Array>,
	patchOffset: number,
	headerOffset: number | null = 12,
) {
	const bytes = valueOf(result);
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	// Read the actual encoder output at the language-neutral protocol's header offsets.
	// Empty graphics has no body, so its encoded length is its fixed header length.
	return format(
		bytes.subarray(0, 4),
		{ major: view.getUint8(4), minor: view.getUint8(5), patch: view.getUint8(patchOffset) },
		headerOffset === null ? bytes.byteLength : view.getUint16(headerOffset, true),
	);
}

it('matches the language-neutral registry with real constants and encoded enum values', async () => {
	const registry: unknown = JSON.parse(
		await readFile(
			new URL('../../../../schemas/terminal/v1/constants.json', import.meta.url),
			'utf8',
		),
	);
	const canonical = CanonicalFrameFixtureSchema.parse(
		JSON.parse(
			await readFile(
				new URL('../../../../schemas/terminal/v1/fixtures/canonical.json', import.meta.url),
				'utf8',
			),
		),
	);
	// A render row window admits only a snapshot, so the canonical delta drops its base and extension.
	const frame = TerminalFrameSchema.parse({
		...canonical.frame,
		baseRevision: null,
		extensions: [],
		kind: TerminalFrameKind.snapshot,
	});
	const graphics = {
		animationTick: 0n,
		images: [],
		kind: TerminalGraphicsUpdateKind.snapshot,
		placements: [],
		resourceRevision: 0n,
	} as const;
	const rowWindow = encodeTerminalRowWindow({
		attachmentId: frame.attachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame,
		generation: frame.generation,
		graphics,
		hostEpoch: frame.hostEpoch,
		revision: frame.revision,
		sequence: frame.sequence,
		startRow: frame.scrollbackRows,
		totalRows: frame.scrollbackRows + BigInt(frame.rows),
	});
	const kinds = Object.values(MessageKind);
	expect(new Set(kinds).size).toBe(kinds.length);
	expect(Object.keys(MESSAGE_CONTRACTS)).toHaveLength(kinds.length);
	const payloadEncodings = new Map<string, number>();
	const messageRoles = new Map<string, number>();
	const messageContracts = Object.entries(MESSAGE_CONTRACTS).map(([kindText, contract]) => {
		const kind = Number(kindText);
		for (const role of ['request', 'response', 'event'] as const) {
			const encoding =
				role === 'request' ? contract.request?.payloadEncoding : contract[`${role}Encoding`];
			if (encoding === null || encoding === undefined) continue;
			const encoded = encodeProtocolEnvelope({
				correlationId: '22222222222222222222222222222222',
				hostEpoch: '11111111111111111111111111111111',
				kind,
				optionalMessage: false,
				payload: new Uint8Array(),
				payloadEncoding: encoding,
				protocol: CURRENT_PROTOCOL_VERSION,
				response: role === 'response',
				event: role === 'event',
			});
			if (!encoded.ok) throw new Error(encoded.detail);
			const view = new DataView(encoded.value.buffer, encoded.value.byteOffset);
			const encodingCode = view.getUint8(22);
			const roleCode = view.getUint16(6, true);
			expect(encodingCode).toBe(payloadEncodings.get(encoding) ?? encodingCode);
			expect(roleCode).toBe(messageRoles.get(role) ?? roleCode);
			payloadEncodings.set(encoding, encodingCode);
			messageRoles.set(role, roleCode);
		}
		return {
			kind,
			name: contract.name,
			event_encoding: contract.eventEncoding,
			response_encoding: contract.responseEncoding,
			request:
				contract.request === null
					? null
					: {
							payload_encoding: contract.request.payloadEncoding,
							success_kind: contract.request.successKind,
						},
		};
	});
	expect(new Set(payloadEncodings.keys())).toEqual(new Set(Object.values(PayloadEncoding)));
	expect(registry).toEqual({
		formats: {
			wire: format(
				TRANSPORT_ENVELOPE_MAGIC,
				CURRENT_PROTOCOL_VERSION,
				TRANSPORT_ENVELOPE_HEADER_BYTES,
			),
			frame: encodedFormat(encodeTerminalFrame(frame), 144),
			projection_buffer: encodedFormat(encodeTerminalProjectionBuffer(frame), 144),
			row_window: encodedFormat(rowWindow, 100),
			graphics: encodedFormat(encodeTerminalGraphics(graphics), 6, null),
		},
		enums: {
			payload_encoding: Object.fromEntries(payloadEncodings),
			message_role: Object.fromEntries(messageRoles),
		},
		message_contracts: messageContracts,
		protocol_limits: Object.fromEntries(
			Object.entries(PROTOCOL_LIMITS).map(([name, value]) => [
				(name.startsWith('max') ? name.slice(3) : name)
					.replaceAll(/([a-z\d])([A-Z])/g, '$1_$2')
					.toLowerCase(),
				value,
			]),
		),
	});
});
