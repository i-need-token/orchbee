import { describe, expect, it } from 'vitest';

import {
	decodeGatewayAttachmentClientMessage,
	decodeGatewayControlClientMessage,
	encodeGatewayAttachmentClientMessage,
	encodeGatewayControlClientMessage,
} from '../gateway-json-codec';
import {
	GatewayAttachmentClientMessageSchema,
	GatewayControlClientMessageSchema,
} from '../gateway-control-contract';

const capability = 'a'.repeat(43);

describe('Gateway JSON codec', () => {
	it('round-trips strict control authentication without weakening the capability', () => {
		const encoded = encodeGatewayControlClientMessage(
			GatewayControlClientMessageSchema.parse({
				capability,
				protocolVersion: '1.1',
				type: 'gateway-control-authenticate',
			}),
		);

		expect(decodeGatewayControlClientMessage(encoded)).toEqual({
			ok: true,
			value: {
				capability,
				protocolVersion: '1.1',
				type: 'gateway-control-authenticate',
			},
		});
	});

	it('round-trips attachment authentication and rejects extra authority', () => {
		const encoded = encodeGatewayAttachmentClientMessage(
			GatewayAttachmentClientMessageSchema.parse({
				attachmentCapability: capability,
				attachmentSocketId: 'gateway:attachment:1',
				protocolVersion: '1.1',
				type: 'gateway-attachment-authenticate',
			}),
		);

		expect(decodeGatewayAttachmentClientMessage(encoded).ok).toBe(true);
		expect(
			decodeGatewayAttachmentClientMessage(
				JSON.stringify({
					attachmentCapability: capability,
					attachmentSocketId: 'gateway:attachment:1',
					lifetime: 'quit',
					protocolVersion: '1.1',
					type: 'gateway-attachment-authenticate',
				}),
			).ok,
		).toBe(false);
	});

	it('rejects malformed and role-confused messages', () => {
		expect(decodeGatewayControlClientMessage('{').ok).toBe(false);
		expect(
			decodeGatewayControlClientMessage(
				JSON.stringify({
					attachmentCapability: capability,
					attachmentSocketId: 'gateway:attachment:1',
					protocolVersion: '1.1',
					type: 'gateway-attachment-authenticate',
				}),
			).ok,
		).toBe(false);
	});
});
