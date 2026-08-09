import { describe, expect, it } from 'vitest';

import {
	isControlRequestKind,
	isInputRequestKind,
	isKnownMessageKind,
	messagePayloadEncoding,
	MESSAGE_CONTRACTS,
	MessageKind,
	PayloadEncoding,
} from '../message-contract';

describe('message contract registry', () => {
	it('contains one contract for every unique registered kind', () => {
		const kinds = Object.values(MessageKind);
		expect(new Set(kinds).size).toBe(kinds.length);
		expect(Object.keys(MESSAGE_CONTRACTS)).toHaveLength(kinds.length);
		for (const kind of kinds) {
			expect(isKnownMessageKind(kind)).toBe(true);
			expect(MESSAGE_CONTRACTS[kind].name).not.toBe('');
		}
		expect(isKnownMessageKind(16)).toBe(false);
	});

	it('makes every request encoding and successful response explicit', () => {
		for (const kind of Object.values(MessageKind)) {
			const contract = MESSAGE_CONTRACTS[kind];
			if (contract.request === null) continue;
			expect(messagePayloadEncoding(contract.request.successKind, 'response')).not.toBeNull();
			expect(isControlRequestKind(kind)).toBe(
				contract.request.payloadEncoding === PayloadEncoding.controlJson,
			);
			expect(isInputRequestKind(kind)).toBe(
				contract.request.payloadEncoding === PayloadEncoding.rawBytes,
			);
		}
		expect(messagePayloadEncoding(MessageKind.writeInput, 'request')).toBe(
			PayloadEncoding.rawBytes,
		);
		expect(messagePayloadEncoding(MessageKind.terminalStarted, 'request')).toBeNull();
		expect(messagePayloadEncoding(MessageKind.selectionChanged, 'response')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(messagePayloadEncoding(MessageKind.selectionChanged, 'event')).toBe(
			PayloadEncoding.controlJson,
		);
	});
});
