import { describe, expect, it } from 'vitest';
import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import {
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
} from '@orchbee/workbench-client-protocol';

import {
	decodeGatewayAttachmentClientFrame,
	decodeGatewayAttachmentServerFrame,
	encodeGatewayAttachmentClientFrame,
	encodeGatewayAttachmentServerFrame,
} from '../gateway-attachment-codec';
import { GATEWAY_LIMITS } from '../gateway-control-contract';

const attachmentId = '11'.repeat(16);

describe('Gateway attachment binary codec', () => {
	it('round-trips a scoped terminal command', () => {
		const command = ProjectedTerminalCommandSchema.parse({
			kind: 'selection-copy',
			payload: { attachmentId, requestSequence: '1' },
		});
		const frame = encodeGatewayAttachmentClientFrame(command);

		expect(decodeGatewayAttachmentClientFrame(frame)).toEqual({
			ok: true,
			value: {
				kind: 'selection-copy',
				payload: { attachmentId, requestSequence: '1' },
			},
		});
	});

	it('preserves terminal frame and selection bytes exactly', () => {
		const terminalFrame = new Uint8Array([1, 2, 3, 4]).buffer;
		const selection = new TextEncoder().encode('selected text').buffer;
		const frameEvent = ProjectedTerminalEventSchema.parse({ frame: terminalFrame, kind: 'frame' });
		const selectionEvent = ProjectedTerminalEventSchema.parse({
			content: selection,
			kind: 'selection-copy-content',
			requestSequence: '9',
		});

		expect(
			decodeGatewayAttachmentServerFrame(encodeGatewayAttachmentServerFrame(frameEvent)),
		).toEqual({ ok: true, value: { frame: terminalFrame, kind: 'frame' } });
		expect(
			decodeGatewayAttachmentServerFrame(encodeGatewayAttachmentServerFrame(selectionEvent)),
		).toEqual({
			ok: true,
			value: { content: selection, kind: 'selection-copy-content', requestSequence: '9' },
		});
	});

	it('round-trips an attachment-scoped bell through the Browser transport', () => {
		const bell = ProjectedTerminalEventSchema.parse({
			event: { attachmentId, sequence: '17' },
			kind: 'bell',
		});

		expect(decodeGatewayAttachmentServerFrame(encodeGatewayAttachmentServerFrame(bell))).toEqual({
			ok: true,
			value: bell,
		});
	});

	it('preserves a correlated typed feature failure through the Browser transport', () => {
		const failure = ProjectedTerminalEventSchema.parse({
			failure: {
				code: 'resize-backpressure',
				correlationId: '55'.repeat(16),
				message: 'The Host resize queue is full.',
				operation: 'resize',
				retryable: true,
			},
			kind: 'failure',
		});

		expect(decodeGatewayAttachmentServerFrame(encodeGatewayAttachmentServerFrame(failure))).toEqual(
			{
				ok: true,
				value: failure,
			},
		);
	});

	it('fits the maximum selection payload within the bounded attachment queue', () => {
		const event = ProjectedTerminalEventSchema.parse({
			content: new ArrayBuffer(PROTOCOL_LIMITS.maxSelectionExportBytes),
			kind: 'selection-copy-content',
			requestSequence: '18446744073709551615',
		});
		const frame = encodeGatewayAttachmentServerFrame(event);

		expect(frame.byteLength).toBeLessThanOrEqual(GATEWAY_LIMITS.maxAttachmentBufferedBytes);
	});

	it('rejects truncation and direction confusion', () => {
		const serverFrame = encodeGatewayAttachmentServerFrame(
			ProjectedTerminalEventSchema.parse({
				byteLength: 4,
				kind: 'selection-copied',
				requestSequence: '1',
			}),
		);

		expect(decodeGatewayAttachmentServerFrame(serverFrame.slice(0, 8)).ok).toBe(false);
		expect(decodeGatewayAttachmentClientFrame(serverFrame).ok).toBe(false);
	});
});
