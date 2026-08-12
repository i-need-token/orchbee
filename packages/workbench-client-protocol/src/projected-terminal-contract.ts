import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	ClearTerminalSchema,
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
	ResetTerminalSchema,
	ResizeTerminalSchema,
	RowsAppliedSchema,
	SelectionChangedSchema,
	SelectionCommandSchema,
	SelectionGestureSchema,
	SetHistoryDemandSchema,
	SetProjectionDemandSchema,
	TerminalExitedSchema,
	TerminalBellSchema,
	TerminalFailedSchema,
	TerminalIdSchema,
	TextInputSchema,
} from '@orchbee/terminal-protocol';
import { z } from 'zod';

const utf8Encoder = new TextEncoder();

const projectedTerminalLinkTargetSchema = z
	.string()
	.min(1)
	.max(PROTOCOL_LIMITS.maxStringBytes)
	.refine(
		(value) => utf8Encoder.encode(value).byteLength <= PROTOCOL_LIMITS.maxStringBytes,
		`Link target exceeds ${PROTOCOL_LIMITS.maxStringBytes} UTF-8 bytes.`,
	);

export const ProjectedTerminalOutcomeSchema = z
	.discriminatedUnion('kind', [
		z.strictObject({ event: TerminalExitedSchema, kind: z.literal('exited') }),
		z.strictObject({ event: TerminalFailedSchema, kind: z.literal('failed') }),
	])
	.nullable();

export const ProjectedAttachmentBootstrapSchema = z
	.strictObject({
		attachmentId: AttachmentIdSchema,
		canInput: z.boolean(),
		canResize: z.boolean(),
		creationId: CreationIdSchema,
		hostEpoch: HostEpochSchema,
		terminalId: TerminalIdSchema,
		terminalOutcome: ProjectedTerminalOutcomeSchema,
	})
	.superRefine((bootstrap, context) => {
		const outcome = bootstrap.terminalOutcome;
		if (outcome === null) return;
		if (bootstrap.canInput || bootstrap.canResize) {
			context.addIssue({
				code: 'custom',
				message: 'A terminal with a final outcome cannot grant mutable capabilities.',
				path: ['terminalOutcome'],
			});
		}
		if (
			outcome.event.identity.creationId !== bootstrap.creationId ||
			outcome.event.identity.hostEpoch !== bootstrap.hostEpoch ||
			outcome.event.identity.terminalId !== bootstrap.terminalId
		) {
			context.addIssue({
				code: 'custom',
				message: 'The terminal outcome identity must match the attachment bootstrap identity.',
				path: ['terminalOutcome', 'event', 'identity'],
			});
		}
	});

export const ProjectedTerminalCommandSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('clear-terminal'), payload: ClearTerminalSchema }),
	z.strictObject({ kind: z.literal('focus-input'), payload: FocusInputSchema }),
	z.strictObject({ kind: z.literal('frame-applied'), payload: FrameAppliedSchema }),
	z.strictObject({ kind: z.literal('key-input'), payload: KeyInputSchema }),
	z.strictObject({ kind: z.literal('mouse-input'), payload: MouseInputSchema }),
	z.strictObject({ kind: z.literal('need-snapshot'), payload: NeedSnapshotSchema }),
	z.strictObject({ kind: z.literal('paste-input'), payload: PasteInputSchema }),
	z.strictObject({ kind: z.literal('read-rows'), payload: ReadRowsSchema }),
	z.strictObject({ kind: z.literal('resize-terminal'), payload: ResizeTerminalSchema }),
	z.strictObject({ kind: z.literal('reset-terminal'), payload: ResetTerminalSchema }),
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
			target: projectedTerminalLinkTargetSchema,
		}),
	}),
	z.strictObject({ kind: z.literal('text-input'), payload: TextInputSchema }),
]);

const ProjectedTerminalFailureSchema = z.strictObject({
	code: z.string().min(1).max(128),
	message: z.string().min(1).max(1024),
	operation: z
		.enum([
			'connection',
			'control',
			'copy',
			'history',
			'input',
			'link',
			'protocol',
			'resize',
			'selection',
		])
		.optional(),
	requestSequence: CanonicalUint64TextSchema.optional(),
	retryable: z.boolean(),
});

const ProjectedTerminalBinarySchema = z.union([
	z.instanceof(ArrayBuffer),
	z.instanceof(Uint8Array),
]);

export const ProjectedTerminalEventSchema = z.discriminatedUnion('kind', [
	z.strictObject({ event: TerminalBellSchema, kind: z.literal('bell') }),
	z.strictObject({ frame: ProjectedTerminalBinarySchema, kind: z.literal('frame') }),
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
	z.strictObject({ kind: z.literal('rows'), rows: ProjectedTerminalBinarySchema }),
	z.strictObject({ event: TerminalExitedSchema, kind: z.literal('terminal-exited') }),
	z.strictObject({ event: TerminalFailedSchema, kind: z.literal('terminal-failed') }),
	z.strictObject({ failure: ProjectedTerminalFailureSchema, kind: z.literal('failure') }),
]);

export type ProjectedAttachmentBootstrap = z.infer<typeof ProjectedAttachmentBootstrapSchema>;
export type ProjectedTerminalOutcome = z.infer<typeof ProjectedTerminalOutcomeSchema>;
export type ProjectedTerminalCommand = z.infer<typeof ProjectedTerminalCommandSchema>;
export type ProjectedTerminalEvent = z.infer<typeof ProjectedTerminalEventSchema>;
export type ProjectedTerminalFailure = z.infer<typeof ProjectedTerminalFailureSchema>;
