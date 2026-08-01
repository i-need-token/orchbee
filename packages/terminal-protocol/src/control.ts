import { z } from 'zod';

import {
	AttachmentIdSchema,
	ClientIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { TerminalPhaseSchema } from './lifecycle';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import { ProtocolVersionRangeSchema, ProtocolVersionSchema } from './version';

const boundedSizeSchema = (maximum: number): z.ZodNumber => z.number().int().min(1).max(maximum);
const utf8Encoder = new TextEncoder();
const boundedTextSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxControlJsonBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxControlJsonBytes,
		`Text exceeds ${PROTOCOL_LIMITS.maxControlJsonBytes} UTF-8 bytes`,
	)
	.refine((value) => !value.includes('\0'), 'Launch text cannot contain NUL');
const gridDimensionSchema = z.number().int().min(1).max(PROTOCOL_LIMITS.maxFrameColumns);
const cellPixelDimensionSchema = z.number().int().min(1).max(PROTOCOL_LIMITS.maxCellPixelDimension);
const rgbSchema = z.number().int().min(0).max(0x00ff_ffff);
const uint16Schema = z.number().int().min(0).max(0xffff);
const uint32Schema = z.number().int().min(0).max(0xffff_ffff);

function validateGrid(
	geometry: { readonly columns: number; readonly rows: number },
	context: z.RefinementCtx,
): void {
	if (geometry.columns * geometry.rows > PROTOCOL_LIMITS.maxFrameCells) {
		context.addIssue({
			code: 'custom',
			message: `Terminal grid exceeds ${PROTOCOL_LIMITS.maxFrameCells} cells`,
			path: ['columns'],
		});
	}
}

function validatePixelGeometry(
	geometry: {
		readonly cellPixelHeight: number;
		readonly cellPixelWidth: number;
		readonly columns: number;
		readonly rows: number;
	},
	context: z.RefinementCtx,
): void {
	validateGrid(geometry, context);
	if (geometry.columns * geometry.cellPixelWidth > 0xffff) {
		context.addIssue({
			code: 'custom',
			message: 'Terminal pixel width exceeds the PTY unsigned 16-bit limit',
			path: ['cellPixelWidth'],
		});
	}
	if (geometry.rows * geometry.cellPixelHeight > 0xffff) {
		context.addIssue({
			code: 'custom',
			message: 'Terminal pixel height exceeds the PTY unsigned 16-bit limit',
			path: ['cellPixelHeight'],
		});
	}
}

export const CanonicalUint64TextSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]{0,19})$/)
	.refine((value) => BigInt(value) <= 0xffffffffffffffffn, 'Value exceeds unsigned 64-bit range')
	.brand<'CanonicalUint64Text'>();

const terminalLaunchShape = {
	arguments: z.array(boundedTextSchema).max(4096),
	columns: gridDimensionSchema,
	currentDirectory: boundedTextSchema.nullable(),
	environment: z
		.array(
			z.strictObject({
				name: z
					.string()
					.min(1)
					.max(32_768)
					.refine((value) => !value.includes('=') && !value.includes('\0')),
				value: boundedTextSchema.nullable(),
			}),
		)
		.max(1024),
	executable: boundedTextSchema.min(1),
	rows: gridDimensionSchema,
} as const;

function validateLaunchEnvironment(
	launch: { readonly environment: readonly { readonly name: string }[] },
	context: z.RefinementCtx,
): void {
	const names = launch.environment.map((entry) => entry.name);
	if (new Set(names).size !== names.length) {
		context.addIssue({
			code: 'custom',
			message: 'Environment delta names must be unique',
			path: ['environment'],
		});
	}
}

export const TerminalLaunchSchema = z
	.strictObject(terminalLaunchShape)
	.superRefine((launch, context) => {
		validateLaunchEnvironment(launch, context);
		validateGrid(launch, context);
	});

const responseTextSchema = z
	.string()
	.max(256)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= 256,
		'Response exceeds 256 UTF-8 bytes',
	)
	.refine((value) => !value.includes('\0'), 'Terminal response cannot contain NUL');

export const TerminalProfileSchema = z.strictObject({
	colorScheme: z.enum(['dark', 'light']),
	defaultBackgroundRgb: rgbSchema,
	defaultCursorRgb: rgbSchema.nullable(),
	defaultForegroundRgb: rgbSchema,
	deviceAttributes: z.strictObject({
		primaryConformanceLevel: uint16Schema,
		primaryFeatures: z.array(uint16Schema).max(64),
		secondaryDeviceType: uint16Schema,
		secondaryFirmwareVersion: uint16Schema,
		secondaryRomCartridge: uint16Schema,
		tertiaryUnitId: uint32Schema,
	}),
	enquiryResponse: responseTextSchema,
	palette: z.array(rgbSchema).length(256),
	versionResponse: responseTextSchema,
});

export const TerminalLaunchV1_1Schema = z
	.strictObject({
		...terminalLaunchShape,
		cellPixelHeight: cellPixelDimensionSchema,
		cellPixelWidth: cellPixelDimensionSchema,
		terminalProfile: TerminalProfileSchema,
	})
	.superRefine((launch, context) => {
		validateLaunchEnvironment(launch, context);
		validatePixelGeometry(launch, context);
		const features = launch.terminalProfile.deviceAttributes.primaryFeatures;
		if (new Set(features).size !== features.length) {
			context.addIssue({
				code: 'custom',
				message: 'Primary device-attribute features must be unique',
				path: ['terminalProfile', 'deviceAttributes', 'primaryFeatures'],
			});
		}
	});

export const CreateTerminalSchema = z.strictObject({
	creationId: CreationIdSchema,
	launch: TerminalLaunchSchema,
});

export const CreateTerminalV1_1Schema = z.strictObject({
	creationId: CreationIdSchema,
	launch: TerminalLaunchV1_1Schema,
});

export const TerminalIdentitySchema = z.strictObject({
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalId: TerminalIdSchema,
});

export const AttachTerminalSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	capabilities: z.strictObject({ input: z.boolean(), read: z.boolean(), resize: z.boolean() }),
	requestResizeLease: z.boolean(),
	terminalId: TerminalIdSchema,
});

export const ResizeTerminalSchema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		columns: gridDimensionSchema,
		rows: gridDimensionSchema,
	})
	.superRefine(validateGrid);

export const ResizeTerminalV1_1Schema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		cellPixelHeight: cellPixelDimensionSchema,
		cellPixelWidth: cellPixelDimensionSchema,
		columns: gridDimensionSchema,
		rows: gridDimensionSchema,
	})
	.superRefine(validatePixelGeometry);

export const FrameAppliedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	generation: CanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	sequence: CanonicalUint64TextSchema,
});

export const NeedSnapshotSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	reason: z.enum([
		'base-mismatch',
		'decoder-rejected',
		'dimensions-mismatch',
		'generation-mismatch',
		'identity-mismatch',
		'sequence-gap',
		'store-reset',
	]),
	sequence: CanonicalUint64TextSchema,
});

export const ProjectionDemandSchema = z.enum(['active', 'paused']);

const projectionDemandShape = {
	attachmentId: AttachmentIdSchema,
	demand: ProjectionDemandSchema,
} as const;

export const SetProjectionDemandSchema = z.strictObject(projectionDemandShape);

export const ProjectionDemandAcceptedSchema = z.strictObject(projectionDemandShape);

export const ProjectionBlockedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	reason: z.enum(['capture-allocation-failed', 'projection-too-large']),
	revision: CanonicalUint64TextSchema,
});

export const EmptyControlSchema = z.strictObject({});

export const TerminalTargetSchema = z.strictObject({ terminalId: TerminalIdSchema });

export const DetachTerminalSchema = z.strictObject({ attachmentId: AttachmentIdSchema });

export const TerminalAcknowledgedSchema = z.strictObject({ terminalId: TerminalIdSchema });

export const AttachmentAcknowledgedSchema = z.strictObject({ attachmentId: AttachmentIdSchema });

export const ExitResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ code: uint32Schema, kind: z.literal('exit-code') }),
	z.strictObject({ kind: z.literal('signal'), signal: uint32Schema.min(1) }),
	z.strictObject({ kind: z.literal('unknown'), value: uint32Schema }),
]);

export const TerminalSummarySchema = z.strictObject({
	columns: gridDimensionSchema,
	exit: ExitResultSchema.nullable(),
	identity: TerminalIdentitySchema,
	phase: TerminalPhaseSchema,
	revision: CanonicalUint64TextSchema,
	rows: gridDimensionSchema,
	terminateRequested: z.boolean(),
});

export const TerminalListSchema = z.strictObject({
	terminals: z.array(TerminalSummarySchema).max(1),
});

export const AttachmentCapabilitiesSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	capabilities: z.strictObject({ input: z.boolean(), read: z.boolean(), resize: z.boolean() }),
	resizeLease: z.boolean(),
	terminalId: TerminalIdSchema,
});

export const InputAcceptedSchema = z.strictObject({
	acceptedBytes: z.number().int().min(0).max(PROTOCOL_LIMITS.maxInputAcceptedBytes),
	attachmentId: AttachmentIdSchema,
});

export const ResizeAcceptedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	columns: gridDimensionSchema,
	revision: CanonicalUint64TextSchema,
	rows: gridDimensionSchema,
});

export const ResizeAcceptedV1_1Schema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	cellPixelHeight: cellPixelDimensionSchema,
	cellPixelWidth: cellPixelDimensionSchema,
	columns: gridDimensionSchema,
	revision: CanonicalUint64TextSchema,
	rows: gridDimensionSchema,
});

export const TerminalStartedSchema = z.strictObject({
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalId: TerminalIdSchema,
});

export const TerminalExitedSchema = z.strictObject({
	exit: ExitResultSchema,
	finalRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
});

export const ProtocolRejectedSchema = z.strictObject({
	clientRange: ProtocolVersionRangeSchema,
	hostRange: ProtocolVersionRangeSchema,
	reason: z.enum(['major-mismatch', 'minor-range-mismatch']),
});

export const TypedFailureSchema = z.strictObject({
	code: z.string().min(1).max(128),
	message: z.string().min(1).max(1024),
	retryable: z.boolean(),
});

export const HelloSchema = z.strictObject({
	clientId: ClientIdSchema,
	frameBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxFrameBytes),
	inputBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxInputBytes),
	protocol: ProtocolVersionRangeSchema,
	rowWindowBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxRowWindowBytes),
	supportedOptionalFeatures: z.array(z.string().min(1).max(64)).max(64),
});

export const HelloAcceptedSchema = z.strictObject({
	frameBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxFrameBytes),
	hostEpoch: HostEpochSchema,
	inputBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxInputBytes),
	protocol: ProtocolVersionSchema,
	rowWindowBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxRowWindowBytes),
	supportedOptionalFeatures: z.array(z.string().min(1).max(64)).max(64),
});

export type Hello = z.infer<typeof HelloSchema>;
export type HelloAccepted = z.infer<typeof HelloAcceptedSchema>;
export type AttachmentAcknowledged = z.infer<typeof AttachmentAcknowledgedSchema>;
export type AttachmentCapabilities = z.infer<typeof AttachmentCapabilitiesSchema>;
export type AttachTerminal = z.infer<typeof AttachTerminalSchema>;
export type CanonicalUint64Text = z.infer<typeof CanonicalUint64TextSchema>;
export type CreateTerminal = z.infer<typeof CreateTerminalSchema>;
export type CreateTerminalV1_1 = z.infer<typeof CreateTerminalV1_1Schema>;
export type DetachTerminal = z.infer<typeof DetachTerminalSchema>;
export type ExitResult = z.infer<typeof ExitResultSchema>;
export type FrameApplied = z.infer<typeof FrameAppliedSchema>;
export type NeedSnapshot = z.infer<typeof NeedSnapshotSchema>;
export type InputAccepted = z.infer<typeof InputAcceptedSchema>;
export type ProtocolRejected = z.infer<typeof ProtocolRejectedSchema>;
export type ProjectionBlocked = z.infer<typeof ProjectionBlockedSchema>;
export type ProjectionDemand = z.infer<typeof ProjectionDemandSchema>;
export type ProjectionDemandAccepted = z.infer<typeof ProjectionDemandAcceptedSchema>;
export type ResizeTerminal = z.infer<typeof ResizeTerminalSchema>;
export type ResizeAccepted = z.infer<typeof ResizeAcceptedSchema>;
export type ResizeAcceptedV1_1 = z.infer<typeof ResizeAcceptedV1_1Schema>;
export type ResizeTerminalV1_1 = z.infer<typeof ResizeTerminalV1_1Schema>;
export type SetProjectionDemand = z.infer<typeof SetProjectionDemandSchema>;
export type TerminalAcknowledged = z.infer<typeof TerminalAcknowledgedSchema>;
export type TerminalExited = z.infer<typeof TerminalExitedSchema>;
export type TerminalIdentity = z.infer<typeof TerminalIdentitySchema>;
export type TerminalLaunch = z.infer<typeof TerminalLaunchSchema>;
export type TerminalLaunchV1_1 = z.infer<typeof TerminalLaunchV1_1Schema>;
export type TerminalProfile = z.infer<typeof TerminalProfileSchema>;
export type TerminalList = z.infer<typeof TerminalListSchema>;
export type TerminalStarted = z.infer<typeof TerminalStartedSchema>;
export type TerminalSummary = z.infer<typeof TerminalSummarySchema>;
export type TerminalTarget = z.infer<typeof TerminalTargetSchema>;
export type TypedFailure = z.infer<typeof TypedFailureSchema>;

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export function decodeControlJson<Output>(
	payload: Uint8Array,
	schema: z.ZodType<Output>,
): ProtocolResult<Output> {
	if (payload.byteLength > PROTOCOL_LIMITS.maxControlJsonBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Control JSON exceeds ${PROTOCOL_LIMITS.maxControlJsonBytes} bytes`,
		);
	}

	let text: string;
	try {
		text = utf8Decoder.decode(payload);
	} catch {
		return protocolFailure(ProtocolErrorCode.invalidUtf8, 'Control JSON is not valid UTF-8');
	}

	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return protocolFailure(ProtocolErrorCode.invalidJson, 'Control payload is not valid JSON');
	}

	const parsed = schema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Control payload failed schema validation: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}
	return protocolSuccess(parsed.data);
}
