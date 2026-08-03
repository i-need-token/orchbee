import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	FocusInputSchema,
	FrameAppliedSchema,
	HistoryChangedSchema,
	HistoryObservedSchema,
	HostEpochSchema,
	KeyInputSchema,
	MouseInputSchema,
	NeedSnapshotSchema,
	PasteInputSchema,
	PROTOCOL_LIMITS,
	ReadRowsSchema,
	ResizeTerminalV1_1Schema,
	RowsAppliedSchema,
	SelectionChangedSchema,
	SelectionCommandSchema,
	SelectionGestureSchema,
	SetHistoryDemandSchema,
	SetProjectionDemandSchema,
	TerminalExitedSchema,
	TerminalIdSchema,
	TextInputSchema,
} from '@orchbee/terminal-protocol';
import { z } from 'zod';

export const ProjectedAttachmentBootstrapSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	canInput: z.boolean().optional(),
	canResize: z.boolean(),
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalExit: TerminalExitedSchema.nullable().optional(),
	terminalId: TerminalIdSchema,
});

export const ProjectedTerminalCommandSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('focus-input'), payload: FocusInputSchema }),
	z.strictObject({ kind: z.literal('frame-applied'), payload: FrameAppliedSchema }),
	z.strictObject({ kind: z.literal('key-input'), payload: KeyInputSchema }),
	z.strictObject({ kind: z.literal('mouse-input'), payload: MouseInputSchema }),
	z.strictObject({ kind: z.literal('need-snapshot'), payload: NeedSnapshotSchema }),
	z.strictObject({ kind: z.literal('paste-input'), payload: PasteInputSchema }),
	z.strictObject({ kind: z.literal('read-rows'), payload: ReadRowsSchema }),
	z.strictObject({ kind: z.literal('resize-terminal'), payload: ResizeTerminalV1_1Schema }),
	z.strictObject({ kind: z.literal('rows-applied'), payload: RowsAppliedSchema }),
	z.strictObject({ kind: z.literal('history-observed'), payload: HistoryObservedSchema }),
	z.strictObject({ kind: z.literal('selection-command'), payload: SelectionCommandSchema }),
	z.strictObject({ kind: z.literal('selection-gesture'), payload: SelectionGestureSchema }),
	z.strictObject({
		kind: z.literal('selection-copy'),
		payload: z.strictObject({
			attachmentId: AttachmentIdSchema,
			requestSequence: CanonicalUint64TextSchema,
		}),
	}),
	z.strictObject({ kind: z.literal('set-history-demand'), payload: SetHistoryDemandSchema }),
	z.strictObject({ kind: z.literal('set-projection-demand'), payload: SetProjectionDemandSchema }),
	z.strictObject({
		kind: z.literal('link-open'),
		payload: z.strictObject({
			attachmentId: AttachmentIdSchema,
			requestSequence: CanonicalUint64TextSchema,
			target: z.string().min(1).max(PROTOCOL_LIMITS.maxStringBytes),
		}),
	}),
	z.strictObject({ kind: z.literal('text-input'), payload: TextInputSchema }),
]);

const ProjectedTerminalFailureSchema = z.strictObject({
	code: z.string().min(1).max(128),
	message: z.string().min(1).max(1024),
	operation: z
		.enum(['connection', 'copy', 'history', 'input', 'link', 'protocol', 'resize', 'selection'])
		.optional(),
	requestSequence: CanonicalUint64TextSchema.optional(),
	retryable: z.boolean(),
});

export const ProjectedTerminalEventSchema = z.discriminatedUnion('kind', [
	z.strictObject({ frame: z.instanceof(ArrayBuffer), kind: z.literal('frame') }),
	z.strictObject({ event: HistoryChangedSchema, kind: z.literal('history-changed') }),
	z.strictObject({
		content: z
			.instanceof(ArrayBuffer)
			.refine(
				(value) => value.byteLength <= PROTOCOL_LIMITS.maxSelectionExportBytes,
				'Selection copy content exceeds the protocol limit.',
			),
		kind: z.literal('selection-copy-content'),
		requestSequence: CanonicalUint64TextSchema,
	}),
	z.strictObject({
		byteLength: z.number().int().min(0).max(PROTOCOL_LIMITS.maxSelectionExportBytes),
		kind: z.literal('selection-copied'),
		requestSequence: CanonicalUint64TextSchema,
	}),
	z.strictObject({ event: SelectionChangedSchema, kind: z.literal('selection-changed') }),
	z.strictObject({ kind: z.literal('link-opened'), requestSequence: CanonicalUint64TextSchema }),
	z.strictObject({ kind: z.literal('rows'), rows: z.instanceof(ArrayBuffer) }),
	z.strictObject({ event: TerminalExitedSchema, kind: z.literal('terminal-exited') }),
	z.strictObject({ failure: ProjectedTerminalFailureSchema, kind: z.literal('failure') }),
]);

export type ProjectedAttachmentBootstrap = z.infer<typeof ProjectedAttachmentBootstrapSchema>;
export type ProjectedTerminalCommand = z.infer<typeof ProjectedTerminalCommandSchema>;
export type ProjectedTerminalEvent = z.infer<typeof ProjectedTerminalEventSchema>;
export type ProjectedTerminalFailure = z.infer<typeof ProjectedTerminalFailureSchema>;
