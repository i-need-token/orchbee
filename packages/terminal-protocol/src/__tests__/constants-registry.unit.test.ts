import { readFile } from 'node:fs/promises';
import { expect, it } from 'vitest';

import {
	encodeProtocolEnvelope,
	TRANSPORT_ENVELOPE_HEADER_BYTES,
	TRANSPORT_ENVELOPE_MAGIC,
} from '../envelope';
import { TERMINAL_FRAME_FORMAT, TERMINAL_FRAME_HEADER_BYTES, TERMINAL_FRAME_MAGIC } from '../frame';
import { PROTOCOL_LIMITS } from '../limits';
import { MESSAGE_CONTRACTS, MessageKind, PayloadEncoding } from '../message-contract';
import {
	TERMINAL_PROJECTION_BUFFER_FORMAT,
	TERMINAL_PROJECTION_BUFFER_HEADER_BYTES,
	TERMINAL_PROJECTION_BUFFER_MAGIC,
} from '../projection-buffer';
import {
	TERMINAL_ROW_WINDOW_FORMAT,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TERMINAL_ROW_WINDOW_MAGIC,
} from '../row-window';
import {
	TERMINAL_GRAPHICS_FORMAT,
	TERMINAL_GRAPHICS_HEADER_BYTES,
	TERMINAL_GRAPHICS_MAGIC,
} from '../terminal-graphics';
import { CURRENT_PROTOCOL_VERSION, type ProtocolVersion } from '../version';

function format(magic: Uint8Array, version: ProtocolVersion, headerBytes: number) {
	return { magic: new TextDecoder().decode(magic), version, header_bytes: headerBytes };
}

it('matches the language-neutral registry with real constants and encoded enum values', async () => {
	const registry: unknown = JSON.parse(
		await readFile(
			new URL('../../../../schemas/terminal/v1/constants.json', import.meta.url),
			'utf8',
		),
	);
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
			frame: format(TERMINAL_FRAME_MAGIC, TERMINAL_FRAME_FORMAT, TERMINAL_FRAME_HEADER_BYTES),
			projection_buffer: format(
				TERMINAL_PROJECTION_BUFFER_MAGIC,
				TERMINAL_PROJECTION_BUFFER_FORMAT,
				TERMINAL_PROJECTION_BUFFER_HEADER_BYTES,
			),
			row_window: format(
				TERMINAL_ROW_WINDOW_MAGIC,
				TERMINAL_ROW_WINDOW_FORMAT,
				TERMINAL_ROW_WINDOW_HEADER_BYTES,
			),
			graphics: format(
				TERMINAL_GRAPHICS_MAGIC,
				TERMINAL_GRAPHICS_FORMAT,
				TERMINAL_GRAPHICS_HEADER_BYTES,
			),
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
