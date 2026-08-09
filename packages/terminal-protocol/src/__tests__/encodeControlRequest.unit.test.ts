import { describe, expect, it } from 'vitest';

import { encodeControlRequest } from '../encodeControlRequest';
import { MessageKind } from '../envelope';
import { ProtocolErrorCode } from '../protocol-result';

describe('control request encoding', () => {
	it('selects the request schema from the message kind', () => {
		expect(encodeControlRequest(MessageKind.ping, {})).toEqual(
			expect.objectContaining({ ok: true }),
		);
		expect(encodeControlRequest(MessageKind.ping, { unexpected: true })).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(
			encodeControlRequest(MessageKind.detachTerminal, {
				attachmentId: '22'.repeat(16),
			}),
		).toEqual(expect.objectContaining({ ok: true }));
		for (const kind of [
			MessageKind.terminateTerminal,
			MessageKind.forceTerminateTerminal,
			MessageKind.disposeTerminal,
		]) {
			expect(encodeControlRequest(kind, { terminalId: '44'.repeat(16) })).toEqual(
				expect.objectContaining({ ok: true }),
			);
		}
	});

	it('rejects response, event, and packed-input kinds', () => {
		for (const kind of [
			MessageKind.helloAccepted,
			MessageKind.terminalStarted,
			MessageKind.writeInput,
		]) {
			expect(encodeControlRequest(kind, {})).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
			);
		}
	});
});
