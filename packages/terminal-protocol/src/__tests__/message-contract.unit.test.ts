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
		expect(MessageKind.startTerminal).toBe(16);
		expect(MessageKind.terminalStartAccepted).toBe(17);
		expect(MessageKind.updateAttachmentInputCapability).toBe(24);
		expect(MessageKind.applyTerminalTheme).toBe(77);
		expect(MessageKind.terminalThemeResult).toBe(78);
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
		expect(MESSAGE_CONTRACTS[MessageKind.startTerminal].request?.successKind).toBe(
			MessageKind.terminalStartAccepted,
		);
		expect(messagePayloadEncoding(MessageKind.terminalStartAccepted, 'response')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(MessageKind.resizeLeaseChanged).toBe(23);
		expect(messagePayloadEncoding(MessageKind.resizeLeaseChanged, 'event')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(messagePayloadEncoding(MessageKind.resizeLeaseChanged, 'request')).toBeNull();
		expect(
			MESSAGE_CONTRACTS[MessageKind.updateAttachmentInputCapability].request?.successKind,
		).toBe(MessageKind.attachmentCapabilities);
		expect(messagePayloadEncoding(MessageKind.selectionChanged, 'response')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(messagePayloadEncoding(MessageKind.selectionChanged, 'event')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(MessageKind.terminalBell).toBe(55);
		expect(messagePayloadEncoding(MessageKind.terminalBell, 'event')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(messagePayloadEncoding(MessageKind.terminalBell, 'request')).toBeNull();
		expect(messagePayloadEncoding(MessageKind.terminalBell, 'response')).toBeNull();
		expect(MessageKind.terminalClipboardWriteRequest).toBe(56);
		expect(messagePayloadEncoding(MessageKind.terminalClipboardWriteRequest, 'event')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(messagePayloadEncoding(MessageKind.terminalClipboardWriteRequest, 'request')).toBeNull();
		expect(MESSAGE_CONTRACTS[MessageKind.clearTerminal].request?.successKind).toBe(
			MessageKind.terminalControlAccepted,
		);
		expect(MESSAGE_CONTRACTS[MessageKind.resetTerminal].request?.successKind).toBe(
			MessageKind.terminalControlAccepted,
		);
		expect(messagePayloadEncoding(MessageKind.terminalControlAccepted, 'response')).toBe(
			PayloadEncoding.controlJson,
		);
		expect(MESSAGE_CONTRACTS[MessageKind.applyTerminalTheme].request?.successKind).toBe(
			MessageKind.terminalThemeResult,
		);
		expect(messagePayloadEncoding(MessageKind.terminalThemeResult, 'response')).toBe(
			PayloadEncoding.controlJson,
		);
	});
});
