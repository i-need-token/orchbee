import type { z } from 'zod';

import {
	GATEWAY_LIMITS,
	GatewayAttachmentClientMessageSchema,
	GatewayAttachmentServerMessageSchema,
	GatewayControlClientMessageSchema,
	GatewayControlServerMessageSchema,
	type GatewayAttachmentClientMessage,
	type GatewayAttachmentServerMessage,
	type GatewayControlClientMessage,
	type GatewayControlServerMessage,
} from './gateway-control-contract';

const textEncoder = new TextEncoder();

export type GatewayDecodeResult<Value> =
	| { readonly ok: true; readonly value: Value }
	| { readonly detail: string; readonly ok: false };

export function decodeGatewayControlClientMessage(
	text: string,
): GatewayDecodeResult<GatewayControlClientMessage> {
	return decodeJson(text, GatewayControlClientMessageSchema);
}

export function decodeGatewayControlServerMessage(
	text: string,
): GatewayDecodeResult<GatewayControlServerMessage> {
	return decodeJson(text, GatewayControlServerMessageSchema);
}

export function decodeGatewayAttachmentClientMessage(
	text: string,
): GatewayDecodeResult<GatewayAttachmentClientMessage> {
	return decodeJson(text, GatewayAttachmentClientMessageSchema);
}

export function decodeGatewayAttachmentServerMessage(
	text: string,
): GatewayDecodeResult<GatewayAttachmentServerMessage> {
	return decodeJson(text, GatewayAttachmentServerMessageSchema);
}

export function encodeGatewayControlClientMessage(message: GatewayControlClientMessage): string {
	return encodeJson(message, GatewayControlClientMessageSchema);
}

export function encodeGatewayControlServerMessage(message: GatewayControlServerMessage): string {
	return encodeJson(message, GatewayControlServerMessageSchema);
}

export function encodeGatewayAttachmentClientMessage(
	message: GatewayAttachmentClientMessage,
): string {
	return encodeJson(message, GatewayAttachmentClientMessageSchema);
}

export function encodeGatewayAttachmentServerMessage(
	message: GatewayAttachmentServerMessage,
): string {
	return encodeJson(message, GatewayAttachmentServerMessageSchema);
}

function decodeJson<Schema extends z.ZodType>(
	text: string,
	schema: Schema,
): GatewayDecodeResult<z.infer<Schema>> {
	if (textEncoder.encode(text).byteLength > GATEWAY_LIMITS.maxControlMessageBytes) {
		return { detail: 'Gateway JSON exceeds the control message limit.', ok: false };
	}
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return { detail: 'Gateway JSON is malformed.', ok: false };
	}
	const parsed = schema.safeParse(value);
	return parsed.success
		? { ok: true, value: parsed.data }
		: { detail: 'Gateway JSON does not match its channel contract.', ok: false };
}

function encodeJson<Schema extends z.ZodType>(value: z.input<Schema>, schema: Schema): string {
	const text = JSON.stringify(schema.parse(value));
	if (textEncoder.encode(text).byteLength > GATEWAY_LIMITS.maxControlMessageBytes) {
		throw new Error('Gateway JSON exceeds the control message limit.');
	}
	return text;
}
