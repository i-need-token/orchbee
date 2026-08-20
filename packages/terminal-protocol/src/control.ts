import { z } from 'zod';

import {
	AttachmentIdSchema,
	ClientIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalThemeOperationIdSchema,
} from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { TerminalPhase } from './terminal-phase';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import { ProtocolVersionSchema, SemanticVersionTextSchema } from './version';
import { TerminalRowWindowDetail } from './row-window';
import { containsUnpairedSurrogate } from './containsUnpairedSurrogate';

const boundedSizeSchema = (maximum: number): z.ZodNumber => z.number().int().min(1).max(maximum);
const utf8Encoder = new TextEncoder();

const boundedTextSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxControlJsonBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxControlJsonBytes,
		`Text exceeds ${PROTOCOL_LIMITS.maxControlJsonBytes} UTF-8 bytes`,
	)
	.refine((value) => !containsUnpairedSurrogate(value), 'Text contains an unpaired surrogate')
	.refine((value) => !value.includes('\0'), 'Launch text cannot contain NUL');
const gridDimensionSchema = z.number().int().min(1).max(PROTOCOL_LIMITS.maxFrameColumns);
const cellPixelDimensionSchema = z.number().int().min(1).max(PROTOCOL_LIMITS.maxCellPixelDimension);
const rgbSchema = z.number().int().min(0).max(0x00ff_ffff);
const uint16Schema = z.number().int().min(0).max(0xffff);
const uint32Schema = z.number().int().min(0).max(0xffff_ffff);
const boundedSelectionPreviewSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxSelectionPreviewBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxSelectionPreviewBytes,
		`Selection preview exceeds ${PROTOCOL_LIMITS.maxSelectionPreviewBytes} UTF-8 bytes`,
	)
	.refine(
		(value) => !containsUnpairedSurrogate(value),
		'Selection preview contains an unpaired surrogate',
	);
const optionalFeatureNameSchema = z
	.string()
	.min(1)
	.max(PROTOCOL_LIMITS.maxOptionalFeatureNameBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxOptionalFeatureNameBytes,
		`Optional feature name exceeds ${PROTOCOL_LIMITS.maxOptionalFeatureNameBytes} UTF-8 bytes`,
	)
	.refine(
		(value) => !containsUnpairedSurrogate(value),
		'Optional feature name contains an unpaired surrogate',
	);

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

const MAXIMUM_UINT64_TEXT = '18446744073709551615';

export const CanonicalUint64TextSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]{0,19})$/)
	.refine(
		(value) =>
			value.length < MAXIMUM_UINT64_TEXT.length ||
			(value.length === MAXIMUM_UINT64_TEXT.length && value <= MAXIMUM_UINT64_TEXT),
		'Value exceeds unsigned 64-bit range',
	)
	.brand<'CanonicalUint64Text'>();

const positiveCanonicalUint64TextSchema = CanonicalUint64TextSchema.refine(
	(value) => value !== '0',
	'Value must be greater than zero',
);

export const TerminalConfigurationSchema = z.strictObject({
	scrollbackBytes: z.number().int().min(0).max(PROTOCOL_LIMITS.maxScrollbackBytes),
});

// Cross-language contract: search for `terminal-scrollback-configuration-contract` before
// changing this value, then update every occurrence in the same change.
export const DEFAULT_TERMINAL_CONFIGURATION = Object.freeze(
	TerminalConfigurationSchema.parse({ scrollbackBytes: 10_000_000 }),
);

const terminalLaunchShape = {
	arguments: z.array(boundedTextSchema).max(PROTOCOL_LIMITS.maxLaunchArguments),
	configuration: TerminalConfigurationSchema.nullable(),
	currentDirectory: boundedTextSchema.nullable(),
	environment: z
		.array(
			z.strictObject({
				name: z
					.string()
					.min(1)
					.max(PROTOCOL_LIMITS.maxEnvironmentNameBytes)
					.refine(
						(value) =>
							utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxEnvironmentNameBytes,
						`Environment name exceeds ${PROTOCOL_LIMITS.maxEnvironmentNameBytes} UTF-8 bytes`,
					)
					.refine(
						(value) => !containsUnpairedSurrogate(value),
						'Environment name contains an unpaired surrogate',
					)
					.refine((value) => !value.includes('=') && !value.includes('\0')),
				value: boundedTextSchema.nullable(),
			}),
		)
		.max(PROTOCOL_LIMITS.maxLaunchEnvironmentEntries),
	executable: boundedTextSchema.min(1),
} as const;

function validateLaunchEnvironment(
	launch: { readonly environment: readonly { readonly name: string }[] },
	context: z.RefinementCtx,
): void {
	// Wire-level identity: two names that are identical exactly as written are
	// one JSON member and cannot be a delta. Names that differ only by case
	// (for example `Path` and `PATH`) remain distinct on the wire; the Host
	// decides their target-operating-system identity when it prepares the
	// launch (see prepareLaunchEnvironment below).
	const names = launch.environment.map((entry) => entry.name);
	if (new Set(names).size !== names.length) {
		context.addIssue({
			code: 'custom',
			message: 'Environment delta names must be unique',
			path: ['environment'],
		});
	}
}

/** A Host operating system that defines environment-name identity. */
export type EnvironmentTarget = 'posix' | 'windows';

/**
 * Environment names are target-operating-system environment identifiers. Fold
 * a name to its target-identity canonical form: POSIX names are case-sensitive
 * (identity is the exact name), while Windows names are case-insensitive
 * (identity is the case-folded name). The ASCII fold here mirrors the native
 * Host canonicalization for the tested corpus; the native implementation is
 * authoritative for non-ASCII codepoints.
 */
export function canonicalEnvironmentName(name: string, target: EnvironmentTarget): string {
	if (target !== 'windows') return name;
	let folded = '';
	for (const character of name) {
		const code = character.charCodeAt(0);
		folded += code >= 0x61 && code <= 0x7a ? String.fromCharCode(code - 0x20) : character;
	}
	return folded;
}

/**
 * Host-side preparation for one declared target OS. The wire schema admits
 * distinct names; this boundary applies the target identity rule and rejects
 * duplicates that only the target OS considers equal. The native Host applies
 * the identical rule and reports a typed `invalid-field` failure, and the
 * `CreationId` launch digest uses the same folded names.
 */
export function prepareLaunchEnvironment(
	environment: readonly { readonly name: string; readonly value: string | null }[],
	target: EnvironmentTarget,
): ProtocolResult<readonly { readonly name: string; readonly value: string | null }[]> {
	const seen = new Set<string>();
	for (const entry of environment) {
		const folded = canonicalEnvironmentName(entry.name, target);
		if (seen.has(folded)) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Duplicate environment name '${entry.name}' for the ${target} target`,
			);
		}
		seen.add(folded);
	}
	return protocolSuccess(environment);
}

const responseTextSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxTerminalResponseBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxTerminalResponseBytes,
		`Response exceeds ${PROTOCOL_LIMITS.maxTerminalResponseBytes} UTF-8 bytes`,
	)
	.refine((value) => !containsUnpairedSurrogate(value), 'Response contains an unpaired surrogate')
	.refine((value) => !value.includes('\0'), 'Terminal response cannot contain NUL');

export const TerminalProfileSchema = z.strictObject({
	colorScheme: z.enum(['dark', 'light']),
	defaultBackgroundRgb: rgbSchema,
	defaultCursorBlinking: z.boolean().default(false),
	defaultCursorRgb: rgbSchema.nullable(),
	defaultCursorShape: z.enum(['block', 'underline', 'bar']).default('block'),
	defaultForegroundRgb: rgbSchema,
	deviceAttributes: z.strictObject({
		primaryConformanceLevel: uint16Schema,
		primaryFeatures: z.array(uint16Schema).max(PROTOCOL_LIMITS.maxTerminalProfileFeatures),
		secondaryDeviceType: uint16Schema,
		secondaryFirmwareVersion: uint16Schema,
		secondaryRomCartridge: uint16Schema,
		tertiaryUnitId: uint32Schema,
	}),
	enquiryResponse: responseTextSchema,
	palette: z.array(rgbSchema).length(PROTOCOL_LIMITS.terminalProfilePaletteEntries),
	versionResponse: responseTextSchema,
});

export const TerminalThemeSchema = z.strictObject({
	colorScheme: z.enum(['dark', 'light']),
	defaultBackgroundRgb: rgbSchema,
	defaultCursorRgb: rgbSchema.nullable(),
	defaultForegroundRgb: rgbSchema,
	palette: z.array(rgbSchema).length(PROTOCOL_LIMITS.terminalProfilePaletteEntries),
});

export const TerminalThemeStateSchema = z
	.strictObject({
		operationId: TerminalThemeOperationIdSchema.nullable(),
		revision: CanonicalUint64TextSchema,
		terminal: TerminalThemeSchema,
	})
	.superRefine((state, context) => {
		if ((state.revision === '0') !== (state.operationId === null)) {
			context.addIssue({
				code: 'custom',
				message: 'Theme revision zero is the only state without an operation ID',
				path: ['operationId'],
			});
		}
	});

export const ApplyTerminalThemeSchema = z.strictObject({
	expectedThemeRevision: CanonicalUint64TextSchema,
	operationId: TerminalThemeOperationIdSchema,
	terminal: TerminalThemeSchema,
	terminalId: TerminalIdSchema,
});

export const TerminalThemeResultSchema = z
	.strictObject({
		current: TerminalThemeStateSchema,
		operationId: TerminalThemeOperationIdSchema,
		terminalId: TerminalIdSchema,
		transition: z.enum(['already-applied', 'applied', 'stale']),
	})
	.superRefine((result, context) => {
		const matchesCurrent = result.current.operationId === result.operationId;
		if (result.transition === 'stale' ? matchesCurrent : !matchesCurrent) {
			context.addIssue({
				code: 'custom',
				message:
					result.transition === 'stale'
						? 'A stale result must identify a different current operation'
						: 'An applied result must identify the current operation',
				path: ['current', 'operationId'],
			});
		}
	});

export const TerminalGeometrySchema = z
	.strictObject({
		cellPixelHeight: cellPixelDimensionSchema,
		cellPixelWidth: cellPixelDimensionSchema,
		columns: gridDimensionSchema,
		rows: gridDimensionSchema,
	})
	.superRefine(validatePixelGeometry);

const immediateTerminalLaunchSchema = z.strictObject({
	...terminalLaunchShape,
	cellPixelHeight: cellPixelDimensionSchema,
	cellPixelWidth: cellPixelDimensionSchema,
	columns: gridDimensionSchema,
	rows: gridDimensionSchema,
	terminalProfile: TerminalProfileSchema,
});

const deferredTerminalLaunchSchema = z.strictObject({
	...terminalLaunchShape,
	cellPixelHeight: z.null(),
	cellPixelWidth: z.null(),
	columns: z.null(),
	rows: z.null(),
	terminalProfile: TerminalProfileSchema,
});

export const TerminalLaunchSchema = z
	.union([immediateTerminalLaunchSchema, deferredTerminalLaunchSchema])
	.superRefine((launch, context) => {
		validateLaunchEnvironment(launch, context);
		if (launch.columns !== null) validatePixelGeometry(launch, context);
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

export const StartTerminalSchema = z.strictObject({
	geometry: TerminalGeometrySchema,
	terminalId: TerminalIdSchema,
});

export const TerminalIdentitySchema = z.strictObject({
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalId: TerminalIdSchema,
});

const attachmentCapabilitiesSchema = z
	.strictObject({ input: z.boolean(), read: z.boolean(), resize: z.boolean() })
	.refine((capabilities) => capabilities.input || capabilities.read || capabilities.resize, {
		message: 'At least one attachment capability is required',
	});

export const AttachTerminalSchema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		capabilities: attachmentCapabilitiesSchema,
		requestResizeLease: z.boolean(),
		terminalId: TerminalIdSchema,
	})
	.superRefine((request, context) => {
		if (request.requestResizeLease && !request.capabilities.resize) {
			context.addIssue({
				code: 'custom',
				message: 'A resize lease requires the resize capability',
				path: ['requestResizeLease'],
			});
		}
	});

export const UpdateAttachmentInputCapabilitySchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	capabilityRevision: positiveCanonicalUint64TextSchema,
	input: z.boolean(),
});

export const ResizeTerminalSchema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		cellPixelHeight: cellPixelDimensionSchema,
		cellPixelWidth: cellPixelDimensionSchema,
		columns: gridDimensionSchema,
		rows: gridDimensionSchema,
	})
	.superRefine(validatePixelGeometry);

const terminalControlTargetShape = {
	attachmentId: AttachmentIdSchema,
} as const;

export const ClearTerminalSchema = z.strictObject(terminalControlTargetShape);
export const ResetTerminalSchema = z.strictObject(terminalControlTargetShape);
export const TerminalControlOperationSchema = z.enum(['clear', 'reset']);

export const FrameAppliedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	sequence: positiveCanonicalUint64TextSchema,
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
	sequence: positiveCanonicalUint64TextSchema,
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

export const RowWindowPurposeSchema = z.enum(['buffer', 'search', 'viewport']);

export const RowWindowOriginSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('absolute'), row: CanonicalUint64TextSchema }),
	z.strictObject({ kind: z.literal('oldest'), offset: uint32Schema }),
	z.strictObject({
		delta: z
			.number()
			.int()
			.min(-PROTOCOL_LIMITS.maxRowWindowRows)
			.max(PROTOCOL_LIMITS.maxRowWindowRows),
		kind: z.literal('committed'),
	}),
	z.strictObject({ kind: z.literal('tail'), rowsBefore: uint32Schema }),
]);

export const ReadRowsSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	detail: z.enum(TerminalRowWindowDetail),
	generation: positiveCanonicalUint64TextSchema,
	maximumBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxRowWindowBytes),
	origin: RowWindowOriginSchema,
	purpose: RowWindowPurposeSchema,
	requestSequence: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	rowCount: z.number().int().min(1).max(PROTOCOL_LIMITS.maxRowWindowRows),
});

export const RowsAppliedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	requestSequence: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
});

export const HistoryDemandSchema = z.enum(['following-bottom', 'viewing-history']);

const historyDemandShape = {
	attachmentId: AttachmentIdSchema,
	demand: HistoryDemandSchema,
} as const;

export const SetHistoryDemandSchema = z.strictObject(historyDemandShape);
export const HistoryDemandAcceptedSchema = z.strictObject(historyDemandShape);

export const HistoryChangedSchema = z.strictObject({
	anchorStatus: z.enum(['evicted', 'none', 'valid']),
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	sequence: positiveCanonicalUint64TextSchema,
	totalRows: CanonicalUint64TextSchema,
});

export const HistoryObservedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	sequence: positiveCanonicalUint64TextSchema,
});

export const TerminalBellSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	sequence: positiveCanonicalUint64TextSchema,
});

const terminalClipboardTextSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxClipboardTextBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxClipboardTextBytes,
		`Clipboard text exceeds ${PROTOCOL_LIMITS.maxClipboardTextBytes} UTF-8 bytes`,
	)
	.refine(
		(value) => !containsUnpairedSurrogate(value),
		'Clipboard text contains an unpaired surrogate',
	)
	.refine((value) => !value.includes('\0'), 'Clipboard text cannot contain NUL');

export const TerminalClipboardWriteRequestSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	sequence: positiveCanonicalUint64TextSchema,
	text: terminalClipboardTextSchema,
});

export const SelectionPointSchema = z.strictObject({
	column: z
		.number()
		.int()
		.min(0)
		.max(PROTOCOL_LIMITS.maxFrameColumns - 1),
	row: CanonicalUint64TextSchema,
});

const selectionGestureShape = {
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
} as const;

const selectionGesturePositionSchema = z.strictObject({
	x: z.number().int().min(-0xffff).max(0xffff),
	y: z.number().int().min(-0xffff).max(0xffff),
});

const selectionGestureGeometrySchema = z.strictObject({
	cellWidth: cellPixelDimensionSchema,
	columns: gridDimensionSchema,
	paddingLeft: uint16Schema,
	screenHeight: boundedSizeSchema(0xffff),
});

export const SelectionGestureSchema = z.discriminatedUnion('action', [
	z.strictObject({
		...selectionGestureShape,
		action: z.literal('start'),
		clickCount: z.number().int().min(1).max(3),
		point: SelectionPointSchema,
		position: selectionGesturePositionSchema,
		selectOutput: z.boolean(),
	}),
	z.strictObject({
		...selectionGestureShape,
		action: z.literal('update'),
		geometry: selectionGestureGeometrySchema,
		point: SelectionPointSchema,
		position: selectionGesturePositionSchema,
		rectangle: z.boolean(),
	}),
	z.strictObject({
		...selectionGestureShape,
		action: z.literal('end'),
		point: SelectionPointSchema,
	}),
	z.strictObject({ ...selectionGestureShape, action: z.literal('cancel') }),
]);

const selectionCommandShape = {
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
} as const;

export const SelectionCommandSchema = z.discriminatedUnion('command', [
	z.strictObject({ ...selectionCommandShape, command: z.literal('clear') }),
	z.strictObject({ ...selectionCommandShape, command: z.literal('select-all') }),
	z.strictObject({
		...selectionCommandShape,
		command: z.literal('set-range'),
		end: SelectionPointSchema,
		rectangle: z.boolean(),
		start: SelectionPointSchema,
	}),
	z.strictObject({
		...selectionCommandShape,
		command: z.literal('select-output'),
		point: SelectionPointSchema,
	}),
]);

export const SelectionChangedSchema = z.discriminatedUnion('state', [
	z.strictObject({ attachmentId: AttachmentIdSchema, state: z.literal('none') }),
	z.strictObject({ attachmentId: AttachmentIdSchema, state: z.literal('evicted') }),
	z.strictObject({
		attachmentId: AttachmentIdSchema,
		end: SelectionPointSchema,
		preview: boundedSelectionPreviewSchema.nullable(),
		rectangle: z.boolean(),
		start: SelectionPointSchema,
		state: z.literal('active'),
	}),
]);

export const SelectionExportFormatSchema = z.literal('plain');

export const SelectionExportSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	exportSequence: positiveCanonicalUint64TextSchema,
	format: SelectionExportFormatSchema,
});

export const SelectionExportStartedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	byteLength: z.number().int().min(0).max(PROTOCOL_LIMITS.maxSelectionExportBytes),
	exportSequence: positiveCanonicalUint64TextSchema,
	format: SelectionExportFormatSchema,
});

export const SelectionExportChunkAppliedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	chunkSequence: uint32Schema,
	exportSequence: positiveCanonicalUint64TextSchema,
});

export const SelectionExportFinishedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	byteLength: z.number().int().min(0).max(PROTOCOL_LIMITS.maxSelectionExportBytes),
	exportSequence: positiveCanonicalUint64TextSchema,
	status: z.enum(['cancelled', 'completed']),
});

export const SelectionExportCancelSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	exportSequence: positiveCanonicalUint64TextSchema,
	reason: z.enum(['clipboard-failed', 'disposed', 'replaced', 'user']),
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

export const RuntimeFailureKind = {
	processControl: 'process-control',
	processObservation: 'process-observation',
	ptyRead: 'pty-read',
	ptyWrite: 'pty-write',
	resize: 'resize',
	terminalReply: 'terminal-reply',
	workerIntegrity: 'worker-integrity',
} as const;

export const RuntimeFailureStage = {
	admitTerminalReply: 'admit-terminal-reply',
	applyResize: 'apply-resize',
	containProcess: 'contain-process',
	dispatchTermination: 'dispatch-termination',
	observeProcess: 'observe-process',
	readOutput: 'read-output',
	releaseOutput: 'release-output',
	releaseProcessIdentity: 'release-process-identity',
	synchronizeWorker: 'synchronize-worker',
	writeInput: 'write-input',
} as const;

const processContainmentFailureSchema = z.strictObject({
	kind: z.literal(RuntimeFailureKind.processControl),
	stage: z.literal(RuntimeFailureStage.containProcess),
});

const processObservationFailureSchema = z.strictObject({
	kind: z.literal(RuntimeFailureKind.processObservation),
	stage: z.literal(RuntimeFailureStage.observeProcess),
});

const outputReleaseFailureSchema = z.strictObject({
	kind: z.literal(RuntimeFailureKind.ptyRead),
	stage: z.literal(RuntimeFailureStage.releaseOutput),
});

const processIdentityReleaseFailureSchema = z.strictObject({
	kind: z.literal(RuntimeFailureKind.processObservation),
	stage: z.literal(RuntimeFailureStage.releaseProcessIdentity),
});

export const RuntimeFailureSchema = z.discriminatedUnion('stage', [
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.terminalReply),
		stage: z.literal(RuntimeFailureStage.admitTerminalReply),
	}),
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.resize),
		stage: z.literal(RuntimeFailureStage.applyResize),
	}),
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.processControl),
		stage: z.literal(RuntimeFailureStage.dispatchTermination),
	}),
	processObservationFailureSchema,
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.ptyRead),
		stage: z.literal(RuntimeFailureStage.readOutput),
	}),
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.workerIntegrity),
		stage: z.literal(RuntimeFailureStage.synchronizeWorker),
	}),
	z.strictObject({
		kind: z.literal(RuntimeFailureKind.ptyWrite),
		stage: z.literal(RuntimeFailureStage.writeInput),
	}),
]);

export const RuntimeCleanupFailureSchema = z.discriminatedUnion('stage', [
	processContainmentFailureSchema,
	outputReleaseFailureSchema,
	processIdentityReleaseFailureSchema,
]);

export const ProcessObservationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('observed'), result: ExitResultSchema }),
	z.strictObject({
		failure: processObservationFailureSchema,
		kind: z.literal('unavailable'),
	}),
]);

export const FailureContainmentSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('stopping'),
		process: ProcessObservationSchema.nullable(),
	}),
	z.strictObject({
		cleanupFailure: RuntimeCleanupFailureSchema,
		kind: z.literal('blocked'),
		process: ProcessObservationSchema.nullable(),
	}),
]);

const terminalSummaryShape = {
	configuration: TerminalConfigurationSchema,
	configurationRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
	revision: CanonicalUint64TextSchema,
	theme: TerminalThemeStateSchema,
} as const;

const terminalSummaryWithGeometryShape = {
	...terminalSummaryShape,
	columns: gridDimensionSchema,
	rows: gridDimensionSchema,
} as const;

const terminalSummaryWithoutFailureShape = {
	...terminalSummaryWithGeometryShape,
	failure: z.null(),
	failureContainment: z.null(),
	process: z.null(),
} as const;

export const TerminalSummarySchema = z.discriminatedUnion('phase', [
	z.strictObject({
		...terminalSummaryShape,
		columns: z.null(),
		exit: z.null(),
		failure: z.null(),
		failureContainment: z.null(),
		phase: z.literal(TerminalPhase.awaitingStart),
		process: z.null(),
		rows: z.null(),
		terminateRequested: z.literal(false),
	}),
	z.strictObject({
		...terminalSummaryWithoutFailureShape,
		exit: z.null(),
		phase: z.literal(TerminalPhase.running),
		terminateRequested: z.boolean(),
	}),
	z.strictObject({
		...terminalSummaryWithoutFailureShape,
		exit: ExitResultSchema.nullable(),
		phase: z.literal(TerminalPhase.finishing),
		terminateRequested: z.boolean(),
	}),
	z.strictObject({
		...terminalSummaryWithGeometryShape,
		exit: z.null(),
		failure: RuntimeFailureSchema,
		failureContainment: FailureContainmentSchema,
		phase: z.literal(TerminalPhase.containingFailure),
		process: z.null(),
		terminateRequested: z.literal(true),
	}),
	z.strictObject({
		...terminalSummaryWithoutFailureShape,
		exit: ExitResultSchema,
		phase: z.literal(TerminalPhase.exited),
		terminateRequested: z.boolean(),
	}),
	z.strictObject({
		...terminalSummaryWithGeometryShape,
		exit: z.null(),
		failure: RuntimeFailureSchema,
		failureContainment: z.null(),
		phase: z.literal(TerminalPhase.failed),
		process: ProcessObservationSchema,
		terminateRequested: z.literal(true),
	}),
]);

export const TerminalInventoryCursorSchema = z.strictObject({
	inventoryGeneration: positiveCanonicalUint64TextSchema,
	offset: CanonicalUint64TextSchema,
});

export const ListTerminalsSchema = z.strictObject({
	cursor: TerminalInventoryCursorSchema.nullable(),
	maximumTerminals: z.number().int().min(1).max(PROTOCOL_LIMITS.maxInventoryPageTerminals),
});

export const TerminalListSchema = z
	.strictObject({
		inventoryGeneration: positiveCanonicalUint64TextSchema,
		nextCursor: TerminalInventoryCursorSchema.nullable(),
		terminals: z.array(TerminalSummarySchema).max(PROTOCOL_LIMITS.maxInventoryPageTerminals),
	})
	.superRefine((value, context) => {
		if (
			value.nextCursor !== null &&
			value.nextCursor.inventoryGeneration !== value.inventoryGeneration
		) {
			context.addIssue({
				code: 'custom',
				message: 'nextCursor must name the response inventory generation',
				path: ['nextCursor', 'inventoryGeneration'],
			});
		}
	});

export const AttachmentCapabilitiesSchema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		capabilityRevision: positiveCanonicalUint64TextSchema,
		capabilities: attachmentCapabilitiesSchema,
		resizeLease: z.boolean(),
		terminalId: TerminalIdSchema,
	})
	.superRefine((response, context) => {
		if (response.resizeLease && !response.capabilities.resize) {
			context.addIssue({
				code: 'custom',
				message: 'A resize lease requires the resize capability',
				path: ['resizeLease'],
			});
		}
	});

export const ResizeLeaseChangedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	resizeLease: z.boolean(),
	terminalId: TerminalIdSchema,
});

export const InputAcceptedSchema = z.strictObject({
	acceptedBytes: z.number().int().min(0).max(PROTOCOL_LIMITS.maxInputAcceptedBytes),
	attachmentId: AttachmentIdSchema,
	capabilityRevision: positiveCanonicalUint64TextSchema,
});

export const ResizeAcceptedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	cellPixelHeight: cellPixelDimensionSchema,
	cellPixelWidth: cellPixelDimensionSchema,
	columns: gridDimensionSchema,
	revision: CanonicalUint64TextSchema,
	rows: gridDimensionSchema,
});

export const TerminalControlAcceptedSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	generation: positiveCanonicalUint64TextSchema,
	operation: TerminalControlOperationSchema,
	revision: CanonicalUint64TextSchema,
	terminalId: TerminalIdSchema,
});

export const TerminalStartedSchema = z.strictObject({
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalId: TerminalIdSchema,
});

export const TerminalStartAcceptedSchema = z.strictObject({
	geometry: TerminalGeometrySchema,
	identity: TerminalIdentitySchema,
	transition: z.enum(['already-started', 'started']),
});

export const TerminalExitedSchema = z.strictObject({
	exit: ExitResultSchema,
	finalRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
});

export const TerminalFailedSchema = z.strictObject({
	failure: RuntimeFailureSchema,
	finalRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
	process: ProcessObservationSchema,
});

export const SetTerminalDefaultsSchema = z.strictObject({
	configuration: TerminalConfigurationSchema,
});

export const TerminalDefaultsAcceptedSchema = z.strictObject({
	configuration: TerminalConfigurationSchema,
	revision: CanonicalUint64TextSchema,
});

export const UpdateTerminalConfigurationSchema = z.strictObject({
	configuration: TerminalConfigurationSchema,
	terminalId: TerminalIdSchema,
});

export const TerminalConfigurationAcceptedSchema = z.strictObject({
	configuration: TerminalConfigurationSchema,
	configurationRevision: CanonicalUint64TextSchema,
	generation: positiveCanonicalUint64TextSchema,
	historyRowsEvicted: CanonicalUint64TextSchema,
	revision: CanonicalUint64TextSchema,
	terminalId: TerminalIdSchema,
});

export const ProtocolRejectedSchema = z.strictObject({
	client: ProtocolVersionSchema,
	host: ProtocolVersionSchema,
	reason: z.literal('version-mismatch'),
});

// Cross-language contract: search for `typed-failure-wire-limit` before
// changing either bound, then update every occurrence together.
const MAXIMUM_TYPED_FAILURE_CODE_BYTES = 128;
const MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES = 1024;
const typedFailureCodeSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_TYPED_FAILURE_CODE_BYTES)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= MAXIMUM_TYPED_FAILURE_CODE_BYTES,
		`Typed failure code exceeds ${MAXIMUM_TYPED_FAILURE_CODE_BYTES} UTF-8 bytes`,
	)
	.refine(
		(value) => !containsUnpairedSurrogate(value),
		'Typed failure code contains an unpaired surrogate',
	);
const typedFailureMessageSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES,
		`Typed failure message exceeds ${MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES} UTF-8 bytes`,
	)
	.refine(
		(value) => !containsUnpairedSurrogate(value),
		'Typed failure message contains an unpaired surrogate',
	);

export const TypedFailureSchema = z.strictObject({
	code: typedFailureCodeSchema,
	message: typedFailureMessageSchema,
	retryable: z.boolean(),
});

export const HelloSchema = z.strictObject({
	clientId: ClientIdSchema,
	frameBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxFrameBytes),
	inputBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxInputBytes),
	protocol: ProtocolVersionSchema,
	rowWindowBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxRowWindowBytes),
	supportedOptionalFeatures: z
		.array(optionalFeatureNameSchema)
		.max(PROTOCOL_LIMITS.maxOptionalFeatures),
});

export const HelloAcceptedSchema = z.strictObject({
	frameBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxFrameBytes),
	hostEpoch: HostEpochSchema,
	hostVersion: SemanticVersionTextSchema,
	inputBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxInputBytes),
	protocol: ProtocolVersionSchema,
	rowWindowBytes: boundedSizeSchema(PROTOCOL_LIMITS.maxRowWindowBytes),
	supportedOptionalFeatures: z
		.array(optionalFeatureNameSchema)
		.max(PROTOCOL_LIMITS.maxOptionalFeatures),
});

export type Hello = z.infer<typeof HelloSchema>;
export type HelloAccepted = z.infer<typeof HelloAcceptedSchema>;
export type ApplyTerminalTheme = z.infer<typeof ApplyTerminalThemeSchema>;
export type AttachmentAcknowledged = z.infer<typeof AttachmentAcknowledgedSchema>;
export type AttachmentCapabilities = z.infer<typeof AttachmentCapabilitiesSchema>;
export type AttachTerminal = z.infer<typeof AttachTerminalSchema>;
export type CanonicalUint64Text = z.infer<typeof CanonicalUint64TextSchema>;
export type ClearTerminal = z.infer<typeof ClearTerminalSchema>;
export type CreateTerminal = z.infer<typeof CreateTerminalSchema>;
export type DetachTerminal = z.infer<typeof DetachTerminalSchema>;
export type ExitResult = z.infer<typeof ExitResultSchema>;
export type FailureContainment = z.infer<typeof FailureContainmentSchema>;
export type FrameApplied = z.infer<typeof FrameAppliedSchema>;
export type NeedSnapshot = z.infer<typeof NeedSnapshotSchema>;
export type InputAccepted = z.infer<typeof InputAcceptedSchema>;

export type UpdateAttachmentInputCapability = z.infer<typeof UpdateAttachmentInputCapabilitySchema>;
export type ListTerminals = z.infer<typeof ListTerminalsSchema>;
export type ProtocolRejected = z.infer<typeof ProtocolRejectedSchema>;
export type ProjectionBlocked = z.infer<typeof ProjectionBlockedSchema>;
export type ProjectionDemand = z.infer<typeof ProjectionDemandSchema>;
export type ProjectionDemandAccepted = z.infer<typeof ProjectionDemandAcceptedSchema>;
export type ReadRows = z.infer<typeof ReadRowsSchema>;
export type ResizeTerminal = z.infer<typeof ResizeTerminalSchema>;
export type ResizeAccepted = z.infer<typeof ResizeAcceptedSchema>;
export type ResizeLeaseChanged = z.infer<typeof ResizeLeaseChangedSchema>;
export type ResetTerminal = z.infer<typeof ResetTerminalSchema>;
export type SetProjectionDemand = z.infer<typeof SetProjectionDemandSchema>;
export type SetTerminalDefaults = z.infer<typeof SetTerminalDefaultsSchema>;
export type StartTerminal = z.infer<typeof StartTerminalSchema>;
export type HistoryChanged = z.infer<typeof HistoryChangedSchema>;
export type HistoryDemand = z.infer<typeof HistoryDemandSchema>;
export type HistoryDemandAccepted = z.infer<typeof HistoryDemandAcceptedSchema>;
export type HistoryObserved = z.infer<typeof HistoryObservedSchema>;
export type TerminalBell = z.infer<typeof TerminalBellSchema>;
export type TerminalClipboardWriteRequest = z.infer<typeof TerminalClipboardWriteRequestSchema>;
export type RowWindowOrigin = z.infer<typeof RowWindowOriginSchema>;
export type RowWindowPurpose = z.infer<typeof RowWindowPurposeSchema>;
export type RowsApplied = z.infer<typeof RowsAppliedSchema>;
export type SelectionChanged = z.infer<typeof SelectionChangedSchema>;
export type SelectionCommand = z.infer<typeof SelectionCommandSchema>;
export type SelectionExport = z.infer<typeof SelectionExportSchema>;
export type SelectionExportCancel = z.infer<typeof SelectionExportCancelSchema>;
export type SelectionExportChunkApplied = z.infer<typeof SelectionExportChunkAppliedSchema>;
export type SelectionExportFinished = z.infer<typeof SelectionExportFinishedSchema>;
export type SelectionExportFormat = z.infer<typeof SelectionExportFormatSchema>;
export type SelectionExportStarted = z.infer<typeof SelectionExportStartedSchema>;
export type SelectionGesture = z.infer<typeof SelectionGestureSchema>;
export type SelectionPoint = z.infer<typeof SelectionPointSchema>;
export type SetHistoryDemand = z.infer<typeof SetHistoryDemandSchema>;
export type TerminalAcknowledged = z.infer<typeof TerminalAcknowledgedSchema>;
export type TerminalExited = z.infer<typeof TerminalExitedSchema>;
export type TerminalFailed = z.infer<typeof TerminalFailedSchema>;
export type TerminalGeometry = z.infer<typeof TerminalGeometrySchema>;
export type TerminalIdentity = z.infer<typeof TerminalIdentitySchema>;
export type TerminalLaunch = z.infer<typeof TerminalLaunchSchema>;
export type TerminalConfiguration = z.infer<typeof TerminalConfigurationSchema>;
export type TerminalConfigurationAccepted = z.infer<typeof TerminalConfigurationAcceptedSchema>;
export type TerminalControlAccepted = z.infer<typeof TerminalControlAcceptedSchema>;
export type TerminalControlOperation = z.infer<typeof TerminalControlOperationSchema>;
export type TerminalDefaultsAccepted = z.infer<typeof TerminalDefaultsAcceptedSchema>;
export type TerminalProfile = z.infer<typeof TerminalProfileSchema>;
export type TerminalList = z.infer<typeof TerminalListSchema>;
export type TerminalInventoryCursor = z.infer<typeof TerminalInventoryCursorSchema>;
export type TerminalStarted = z.infer<typeof TerminalStartedSchema>;
export type TerminalStartAccepted = z.infer<typeof TerminalStartAcceptedSchema>;
export type TerminalSummary = z.infer<typeof TerminalSummarySchema>;
export type TerminalTarget = z.infer<typeof TerminalTargetSchema>;
export type TerminalTheme = z.infer<typeof TerminalThemeSchema>;
export type TerminalThemeResult = z.infer<typeof TerminalThemeResultSchema>;
export type TerminalThemeState = z.infer<typeof TerminalThemeStateSchema>;
export type TypedFailure = z.infer<typeof TypedFailureSchema>;
export type ProcessObservation = z.infer<typeof ProcessObservationSchema>;
export type RuntimeFailure = z.infer<typeof RuntimeFailureSchema>;
export type RuntimeCleanupFailure = z.infer<typeof RuntimeCleanupFailureSchema>;
export type UpdateTerminalConfiguration = z.infer<typeof UpdateTerminalConfigurationSchema>;

const utf8Decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

export function encodeControlJson<Output>(
	value: unknown,
	schema: z.ZodType<Output>,
): ProtocolResult<Uint8Array> {
	const parsed = schema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Control value failed schema validation: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}

	let text: string | undefined;
	try {
		text = JSON.stringify(parsed.data);
	} catch {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Control value is not JSON serializable',
		);
	}
	if (text === undefined || containsUnpairedSurrogateInJson(text)) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Control value does not contain only Unicode scalar strings',
		);
	}
	const encoded = utf8Encoder.encode(text);
	if (encoded.byteLength > PROTOCOL_LIMITS.maxControlJsonBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Control JSON exceeds ${PROTOCOL_LIMITS.maxControlJsonBytes} bytes`,
		);
	}
	return protocolSuccess(encoded);
}

type JsonObjectFrame = {
	kind: 'object';
	state: 'key-or-end' | 'colon' | 'value' | 'comma-or-end';
	keys: Set<string>;
};

type JsonArrayFrame = {
	kind: 'array';
	state: 'value-or-end' | 'comma-or-end';
};

type JsonFrame = JsonObjectFrame | JsonArrayFrame;

function isJsonWhitespace(code: number): boolean {
	return code === 0x20 || code === 0x09 || code === 0x0a || code === 0x0d;
}

function isJsonDigit(character: string | undefined): boolean {
	return character !== undefined && character >= '0' && character <= '9';
}

function skipJsonWhitespace(text: string, index: number): number {
	while (index < text.length && isJsonWhitespace(text.charCodeAt(index))) index += 1;
	return index;
}

function scanJsonString(text: string, start: number): number | undefined {
	if (text.charCodeAt(start) !== 0x22) return undefined;
	let index = start + 1;
	while (index < text.length) {
		const code = text.charCodeAt(index);
		if (code < 0x20) return undefined;
		if (code === 0x22) return index + 1;
		if (code === 0x5c) {
			index += 2;
			continue;
		}
		index += 1;
	}
	return undefined;
}

function scanJsonNumber(text: string, start: number): number | undefined {
	let index = start;
	if (text[index] === '-') index += 1;
	const firstDigit = text[index];
	if (firstDigit === '0') {
		index += 1;
	} else if (firstDigit !== undefined && firstDigit >= '1' && firstDigit <= '9') {
		index += 1;
		while (isJsonDigit(text[index])) index += 1;
	} else {
		return undefined;
	}
	if (text[index] === '.') {
		index += 1;
		const fractionStart = index;
		while (isJsonDigit(text[index])) index += 1;
		if (index === fractionStart) return undefined;
	}
	if (text[index] === 'e' || text[index] === 'E') {
		index += 1;
		if (text[index] === '+' || text[index] === '-') index += 1;
		const exponentStart = index;
		while (isJsonDigit(text[index])) index += 1;
		if (index === exponentStart) return undefined;
	}
	return index;
}

function scanJsonScalar(text: string, start: number): number | undefined {
	const stringEnd = scanJsonString(text, start);
	if (stringEnd !== undefined) return stringEnd;
	const numberEnd = scanJsonNumber(text, start);
	if (numberEnd !== undefined) return numberEnd;
	for (const literal of ['true', 'false', 'null']) {
		if (text.startsWith(literal, start)) return start + literal.length;
	}
	return undefined;
}

/**
 * JSON.parse discards duplicate object members before Zod sees them. The
 * native decoder rejects them, so scan object keys first and compare their
 * decoded values (including escaped-equivalent spellings) for parity.
 */
function duplicateJsonObjectMember(text: string): string | undefined {
	const stack: JsonFrame[] = [];
	let index = 0;
	let rootComplete = false;

	while (true) {
		if (stack.length === 0) {
			if (rootComplete) return undefined;
			index = skipJsonWhitespace(text, index);
			const code = text.charCodeAt(index);
			if (code === 0x7b) {
				stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
				index += 1;
				continue;
			}
			if (code === 0x5b) {
				stack.push({ kind: 'array', state: 'value-or-end' });
				index += 1;
				continue;
			}
			const scalarEnd = scanJsonScalar(text, index);
			if (scalarEnd === undefined) return undefined;
			index = scalarEnd;
			rootComplete = true;
			continue;
		}

		const frame = stack[stack.length - 1];
		if (frame === undefined) return undefined;
		if (frame.kind === 'object') {
			switch (frame.state) {
				case 'key-or-end': {
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x7d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					const keyEnd = scanJsonString(text, index);
					if (keyEnd === undefined) return undefined;
					let key: unknown;
					try {
						key = JSON.parse(text.slice(index, keyEnd));
					} catch {
						return undefined;
					}
					if (typeof key !== 'string') return undefined;
					if (frame.keys.has(key)) return 'Control JSON contains a duplicate object member';
					frame.keys.add(key);
					index = keyEnd;
					frame.state = 'colon';
					continue;
				}
				case 'colon':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) !== 0x3a) return undefined;
					index += 1;
					frame.state = 'value';
					continue;
				case 'value': {
					index = skipJsonWhitespace(text, index);
					const code = text.charCodeAt(index);
					frame.state = 'comma-or-end';
					if (code === 0x7b) {
						stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
						index += 1;
						continue;
					}
					if (code === 0x5b) {
						stack.push({ kind: 'array', state: 'value-or-end' });
						index += 1;
						continue;
					}
					const scalarEnd = scanJsonScalar(text, index);
					if (scalarEnd === undefined) return undefined;
					index = scalarEnd;
					continue;
				}
				case 'comma-or-end':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x2c) {
						index += 1;
						frame.state = 'key-or-end';
						continue;
					}
					if (text.charCodeAt(index) === 0x7d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					return undefined;
			}
		} else {
			switch (frame.state) {
				case 'value-or-end': {
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x5d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					const code = text.charCodeAt(index);
					frame.state = 'comma-or-end';
					if (code === 0x7b) {
						stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
						index += 1;
						continue;
					}
					if (code === 0x5b) {
						stack.push({ kind: 'array', state: 'value-or-end' });
						index += 1;
						continue;
					}
					const scalarEnd = scanJsonScalar(text, index);
					if (scalarEnd === undefined) return undefined;
					index = scalarEnd;
					continue;
				}
				case 'comma-or-end':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x2c) {
						index += 1;
						frame.state = 'value-or-end';
						continue;
					}
					if (text.charCodeAt(index) === 0x5d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					return undefined;
			}
		}
	}
}

function containsUnpairedSurrogateInJson(text: string): boolean {
	let index = 0;
	while (index < text.length) {
		if (text.charCodeAt(index) !== 0x22) {
			index += 1;
			continue;
		}
		const end = scanJsonString(text, index);
		if (end === undefined) return false;
		try {
			const value: unknown = JSON.parse(text.slice(index, end));
			if (typeof value === 'string' && containsUnpairedSurrogate(value)) return true;
		} catch {
			return false;
		}
		index = end;
	}
	return false;
}

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

	const duplicateMember = duplicateJsonObjectMember(text);
	if (duplicateMember !== undefined) {
		return protocolFailure(ProtocolErrorCode.invalidJson, duplicateMember);
	}
	if (containsUnpairedSurrogateInJson(text)) {
		return protocolFailure(
			ProtocolErrorCode.invalidJson,
			'Control JSON contains an unpaired surrogate',
		);
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
