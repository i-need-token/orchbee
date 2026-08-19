import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import {
	TerminalProjectionControlCommandSchema,
	TerminalProjectionControlEventSchema,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
} from '@orchbee/workbench-client-protocol';
import { WorkbenchSharedDocumentSchema } from '@orchbee/workbench/document';
import { z } from 'zod';

import { GATEWAY_ATTACHMENT_MAX_FRAME_OVERHEAD_BYTES } from './gateway-attachment-wire-contract';

export const GATEWAY_PROTOCOL_VERSION = '1.2';
export const GATEWAY_CONTROL_PATH = '/v1/control';
export const GATEWAY_ATTACHMENT_PATH = '/v1/attachment';

export const GATEWAY_LIMITS = {
	maxAttachmentBufferedBytes:
		PROTOCOL_LIMITS.maxSelectionExportBytes + GATEWAY_ATTACHMENT_MAX_FRAME_OVERHEAD_BYTES,
	maxAttachmentJsonBytes: 2 * 1024 * 1024,
	maxControlBufferedBytes: 16 * 1024 * 1024,
	maxControlMessageBytes: 16 * 1024 * 1024,
} as const;

export const GatewayCapabilitySchema = z
	.string()
	.length(43)
	.regex(/^[A-Za-z0-9_-]+$/u)
	.brand<'GatewayCapability'>();

export const GatewayClientConnectionIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'GatewayClientConnectionId'>();

export const GatewayAttachmentSocketIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'GatewayAttachmentSocketId'>();

export const GatewayBindHostSchema = z.enum(['127.0.0.1', '0.0.0.0']);

export const GatewayBrowserConnectionSchema = z.strictObject({
	capability: GatewayCapabilitySchema,
	port: z.number().int().min(1).max(65_535),
	protocolVersion: z.literal(GATEWAY_PROTOCOL_VERSION),
});

const gatewayWorkbenchClientSessionBootstrapSchema = WorkbenchClientSessionBootstrapSchema.extend({
	initializationDocument: WorkbenchSharedDocumentSchema,
});

export const GatewayDesktopCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		allowedOrigins: z.array(z.string().min(1).max(2048)).min(1).max(32),
		bindHost: GatewayBindHostSchema,
		capability: GatewayCapabilitySchema,
		type: z.literal('gateway-start'),
	}),
	z.strictObject({
		bootstrap: gatewayWorkbenchClientSessionBootstrapSchema,
		connectionId: GatewayClientConnectionIdSchema,
		type: z.literal('gateway-client-opened'),
	}),
	z.strictObject({
		connectionId: GatewayClientConnectionIdSchema,
		reason: z.enum(['client-session-unavailable', 'desktop-quiescing']),
		type: z.literal('gateway-client-open-failed'),
	}),
	z.strictObject({ type: z.literal('gateway-stop') }),
]);

export const GatewayUtilityEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		port: z.number().int().min(1).max(65_535),
		type: z.literal('gateway-listening'),
	}),
	z.strictObject({
		connectionId: GatewayClientConnectionIdSchema,
		type: z.literal('gateway-client-open-requested'),
	}),
	z.strictObject({
		connectionId: GatewayClientConnectionIdSchema,
		type: z.literal('gateway-client-close-requested'),
	}),
	z.strictObject({
		code: z.enum(['invalid-configuration', 'listen-failed', 'parent-protocol-rejected']),
		type: z.literal('gateway-failed'),
	}),
	z.strictObject({ type: z.literal('gateway-stopped') }),
]);

export const GatewayControlClientMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		capability: GatewayCapabilitySchema,
		protocolVersion: z.literal(GATEWAY_PROTOCOL_VERSION),
		type: z.literal('gateway-control-authenticate'),
	}),
	z.strictObject({
		command: WorkbenchClientSessionCommandSchema,
		type: z.literal('workbench-client-session-command'),
	}),
	z.strictObject({
		command: TerminalProjectionControlCommandSchema,
		type: z.literal('terminal-projection-control-command'),
	}),
]);

const projectionControlEventSchema = TerminalProjectionControlEventSchema.refine(
	(event) => event.type !== 'terminal-projection-opened',
	'The generic Gateway projection event must not transfer an opened attachment.',
);
const projectionOpenedEventSchema = TerminalProjectionControlEventSchema.refine(
	(event) => event.type === 'terminal-projection-opened',
	'The Gateway projection-opened message must contain an opened projection.',
);

export const GatewayControlServerMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		bootstrap: gatewayWorkbenchClientSessionBootstrapSchema,
		type: z.literal('gateway-control-ready'),
	}),
	z.strictObject({
		event: WorkbenchClientSessionEventSchema,
		type: z.literal('workbench-client-session-event'),
	}),
	z.strictObject({
		event: projectionControlEventSchema,
		type: z.literal('terminal-projection-control-event'),
	}),
	z.strictObject({
		attachmentCapability: GatewayCapabilitySchema,
		attachmentSocketId: GatewayAttachmentSocketIdSchema,
		projection: projectionOpenedEventSchema,
		type: z.literal('terminal-projection-opened'),
	}),
	z.strictObject({
		code: z.enum([
			'authentication-rejected',
			'client-session-unavailable',
			'control-backpressure',
			'protocol-rejected',
		]),
		retryable: z.boolean(),
		type: z.literal('gateway-control-failed'),
	}),
]);

export const GatewayAttachmentClientMessageSchema = z.strictObject({
	attachmentCapability: GatewayCapabilitySchema,
	attachmentSocketId: GatewayAttachmentSocketIdSchema,
	protocolVersion: z.literal(GATEWAY_PROTOCOL_VERSION),
	type: z.literal('gateway-attachment-authenticate'),
});

export const GatewayAttachmentServerMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		attachmentSocketId: GatewayAttachmentSocketIdSchema,
		type: z.literal('gateway-attachment-ready'),
	}),
	z.strictObject({
		code: z.enum(['authentication-rejected', 'attachment-retired', 'protocol-rejected']),
		retryable: z.boolean(),
		type: z.literal('gateway-attachment-failed'),
	}),
]);

export type GatewayAttachmentClientMessage = z.infer<typeof GatewayAttachmentClientMessageSchema>;
export type GatewayAttachmentServerMessage = z.infer<typeof GatewayAttachmentServerMessageSchema>;
export type GatewayBindHost = z.infer<typeof GatewayBindHostSchema>;
export type GatewayBrowserConnection = z.infer<typeof GatewayBrowserConnectionSchema>;
export type GatewayCapability = z.infer<typeof GatewayCapabilitySchema>;
export type GatewayClientConnectionId = z.infer<typeof GatewayClientConnectionIdSchema>;
export type GatewayControlClientMessage = z.infer<typeof GatewayControlClientMessageSchema>;
export type GatewayControlServerMessage = z.infer<typeof GatewayControlServerMessageSchema>;
export type GatewayDesktopCommand = z.infer<typeof GatewayDesktopCommandSchema>;
export type GatewayUtilityEvent = z.infer<typeof GatewayUtilityEventSchema>;
