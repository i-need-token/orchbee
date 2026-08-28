import { z } from 'zod';

const MAX_CONTEXTS = 128;
const MAX_IDENTIFIER_LENGTH = 256;
const MAX_SCREEN_COORDINATE = 10_000_000;
const MAX_TABS = 4_096;
const MAX_WINDOWS = 64;
const MAX_WINDOW_ORDINAL = 1_000_000;

const identifierSchema = z
	.string()
	.min(1)
	.max(MAX_IDENTIFIER_LENGTH)
	.regex(/^[A-Za-z0-9:._-]+$/u);

export const DesktopWindowLayoutWindowIdSchema =
	identifierSchema.brand<'DesktopWindowLayoutWindowId'>();
export const DesktopWindowLayoutRequestIdSchema =
	identifierSchema.brand<'DesktopWindowLayoutRequestId'>();
export const DesktopWindowLayoutDragSessionIdSchema =
	identifierSchema.brand<'DesktopWindowLayoutDragSessionId'>();

const terminalContextIdSchema = z.string().min(1).max(MAX_IDENTIFIER_LENGTH);
const tabIdSchema = z.string().min(1).max(MAX_IDENTIFIER_LENGTH);
const groupIdSchema = z.string().min(1).max(MAX_IDENTIFIER_LENGTH);
const uniqueTabIdsSchema = z
	.array(tabIdSchema)
	.min(1)
	.max(MAX_TABS)
	.refine((tabIds) => new Set(tabIds).size === tabIds.length, 'Tab identities must be unique.');

export const DesktopWindowLayoutDestinationSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('existing'),
		windowId: DesktopWindowLayoutWindowIdSchema,
	}),
	z.strictObject({ kind: z.literal('new') }),
]);

const windowSummarySchema = z.strictObject({
	ordinal: z.number().int().min(1).max(MAX_WINDOW_ORDINAL),
	windowId: DesktopWindowLayoutWindowIdSchema,
});
const assignmentSchema = z.strictObject({
	tabIds: z.array(tabIdSchema).max(MAX_TABS),
	terminalContextId: terminalContextIdSchema,
});

export const DesktopWindowLayoutSnapshotSchema = z
	.strictObject({
		currentWindowId: DesktopWindowLayoutWindowIdSchema,
		revision: z.string().regex(/^(0|[1-9][0-9]*)$/u),
		terminalContexts: z.array(assignmentSchema).max(MAX_CONTEXTS),
		windows: z.array(windowSummarySchema).min(1).max(MAX_WINDOWS),
	})
	.superRefine((snapshot, context) => {
		const windowIds = snapshot.windows.map((window) => window.windowId);
		if (new Set(windowIds).size !== windowIds.length) {
			context.addIssue({ code: 'custom', message: 'Window identities must be unique.' });
		}
		if (!windowIds.includes(snapshot.currentWindowId)) {
			context.addIssue({ code: 'custom', message: 'The current window must be listed.' });
		}
		const ordinals = snapshot.windows.map((window) => window.ordinal);
		if (new Set(ordinals).size !== ordinals.length) {
			context.addIssue({ code: 'custom', message: 'Window ordinals must be unique.' });
		}
		const terminalContextIds = snapshot.terminalContexts.map(
			(terminalContext) => terminalContext.terminalContextId,
		);
		if (new Set(terminalContextIds).size !== terminalContextIds.length) {
			context.addIssue({ code: 'custom', message: 'Terminal-context identities must be unique.' });
		}
		const tabIds = snapshot.terminalContexts.flatMap((terminalContext) => terminalContext.tabIds);
		if (new Set(tabIds).size !== tabIds.length) {
			context.addIssue({ code: 'custom', message: 'Tab assignments must be unique.' });
		}
	});

const screenPointSchema = z.strictObject({
	x: z.number().finite().min(-MAX_SCREEN_COORDINATE).max(MAX_SCREEN_COORDINATE),
	y: z.number().finite().min(-MAX_SCREEN_COORDINATE).max(MAX_SCREEN_COORDINATE),
});

const requestIdentityShape = {
	requestId: DesktopWindowLayoutRequestIdSchema,
} as const;
const tabOperationShape = {
	destination: DesktopWindowLayoutDestinationSchema,
	tabIds: uniqueTabIdsSchema,
	terminalContextId: terminalContextIdSchema,
} as const;
const dragIdentityShape = {
	sessionId: DesktopWindowLayoutDragSessionIdSchema,
} as const;

export const DesktopWindowLayoutRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		terminalContextId: terminalContextIdSchema,
		type: z.literal('desktop-window-layout-active-context-changed'),
	}),
	z.strictObject({
		...requestIdentityShape,
		...tabOperationShape,
		type: z.literal('desktop-window-layout-move-tabs-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		destination: DesktopWindowLayoutDestinationSchema,
		groupId: groupIdSchema,
		terminalContextId: terminalContextIdSchema,
		type: z.literal('desktop-window-layout-move-group-requested'),
	}),
	z.strictObject({
		...dragIdentityShape,
		tabIds: uniqueTabIdsSchema,
		terminalContextId: terminalContextIdSchema,
		type: z.literal('desktop-window-layout-drag-started'),
	}),
	z.strictObject({
		...dragIdentityShape,
		point: screenPointSchema,
		type: z.literal('desktop-window-layout-drag-updated'),
	}),
	z.strictObject({
		...dragIdentityShape,
		type: z.literal('desktop-window-layout-drag-cancelled'),
	}),
	z.strictObject({
		...dragIdentityShape,
		point: screenPointSchema,
		type: z.literal('desktop-window-layout-drag-completed'),
	}),
]);

export const DesktopWindowLayoutFailureCodeSchema = z.enum([
	'busy',
	'control-unavailable',
	'native-window-unavailable',
	'persistence',
	'stale-group',
	'stale-tab',
	'stale-window',
	'window-capacity',
]);

export const DesktopWindowLayoutOperationResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		destinationWindowId: DesktopWindowLayoutWindowIdSchema,
		kind: z.literal('completed'),
	}),
	z.strictObject({ kind: z.literal('unchanged') }),
	z.strictObject({
		code: DesktopWindowLayoutFailureCodeSchema,
		kind: z.literal('failed'),
	}),
]);

const dragTargetSchema = z.union([DesktopWindowLayoutDestinationSchema, z.null()]);

export const DesktopWindowLayoutMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		snapshot: DesktopWindowLayoutSnapshotSchema,
		type: z.literal('desktop-window-layout-snapshot-published'),
	}),
	z.strictObject({
		...requestIdentityShape,
		result: DesktopWindowLayoutOperationResultSchema,
		type: z.literal('desktop-window-layout-operation-responded'),
	}),
	z.strictObject({
		...dragIdentityShape,
		target: dragTargetSchema,
		type: z.literal('desktop-window-layout-drag-target-changed'),
	}),
	z.strictObject({
		...dragIdentityShape,
		result: DesktopWindowLayoutOperationResultSchema,
		type: z.literal('desktop-window-layout-drag-settled'),
	}),
]);

export type DesktopWindowLayoutDestination = z.infer<typeof DesktopWindowLayoutDestinationSchema>;
export type DesktopWindowLayoutDragSessionId = z.infer<
	typeof DesktopWindowLayoutDragSessionIdSchema
>;
export type DesktopWindowLayoutFailureCode = z.infer<typeof DesktopWindowLayoutFailureCodeSchema>;
export type DesktopWindowLayoutMainEvent = z.infer<typeof DesktopWindowLayoutMainEventSchema>;
export type DesktopWindowLayoutOperationResult = z.infer<
	typeof DesktopWindowLayoutOperationResultSchema
>;
export type DesktopWindowLayoutRendererCommand = z.infer<
	typeof DesktopWindowLayoutRendererCommandSchema
>;
export type DesktopWindowLayoutRequestId = z.infer<typeof DesktopWindowLayoutRequestIdSchema>;
export type DesktopWindowLayoutSnapshot = z.infer<typeof DesktopWindowLayoutSnapshotSchema>;
export type DesktopWindowLayoutWindowId = z.infer<typeof DesktopWindowLayoutWindowIdSchema>;
