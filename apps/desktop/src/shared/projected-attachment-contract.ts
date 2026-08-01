import {
	AttachmentIdSchema,
	CreationIdSchema,
	FocusInputSchema,
	FrameAppliedSchema,
	HostEpochSchema,
	KeyInputSchema,
	MouseInputSchema,
	NeedSnapshotSchema,
	PasteInputSchema,
	ResizeTerminalV1_1Schema,
	SetProjectionDemandSchema,
	TerminalExitedSchema,
	TerminalIdSchema,
	TextInputSchema,
} from '@orchbee/terminal-protocol';
import { z } from 'zod';

export const PROJECTED_ATTACHMENT_IPC_CHANNEL = 'orchbee.desktop.projected-attachment-port';
export const PROJECTED_ATTACHMENT_WINDOW_MESSAGE = 'orchbee.desktop.projected-attachment-ready';

export const ProjectedAttachmentBootstrapSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	canResize: z.boolean(),
	creationId: CreationIdSchema,
	hostEpoch: HostEpochSchema,
	terminalId: TerminalIdSchema,
});

export const ProjectedAttachmentWindowMessageSchema = z.strictObject({
	bootstrap: ProjectedAttachmentBootstrapSchema,
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(PROJECTED_ATTACHMENT_WINDOW_MESSAGE),
});

export const DesktopRendererCommandSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('focus-input'), payload: FocusInputSchema }),
	z.strictObject({ kind: z.literal('frame-applied'), payload: FrameAppliedSchema }),
	z.strictObject({ kind: z.literal('key-input'), payload: KeyInputSchema }),
	z.strictObject({ kind: z.literal('mouse-input'), payload: MouseInputSchema }),
	z.strictObject({ kind: z.literal('need-snapshot'), payload: NeedSnapshotSchema }),
	z.strictObject({ kind: z.literal('paste-input'), payload: PasteInputSchema }),
	z.strictObject({ kind: z.literal('resize-terminal'), payload: ResizeTerminalV1_1Schema }),
	z.strictObject({
		kind: z.literal('set-projection-demand'),
		payload: SetProjectionDemandSchema,
	}),
	z.strictObject({ kind: z.literal('text-input'), payload: TextInputSchema }),
]);

const DesktopBrokerFailureSchema = z.strictObject({
	code: z.string().min(1).max(128),
	message: z.string().min(1).max(1024),
	retryable: z.boolean(),
});

export const DesktopRendererEventSchema = z.discriminatedUnion('kind', [
	z.strictObject({ frame: z.instanceof(ArrayBuffer), kind: z.literal('frame') }),
	z.strictObject({ event: TerminalExitedSchema, kind: z.literal('terminal-exited') }),
	z.strictObject({ failure: DesktopBrokerFailureSchema, kind: z.literal('failure') }),
]);

export type DesktopBrokerFailure = z.infer<typeof DesktopBrokerFailureSchema>;
export type DesktopRendererCommand = z.infer<typeof DesktopRendererCommandSchema>;
export type ProjectedAttachmentBootstrap = z.infer<typeof ProjectedAttachmentBootstrapSchema>;
