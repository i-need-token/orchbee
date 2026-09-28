import { z } from 'zod';
import {
	TerminalPresentationIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPresentationBindingSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';
import { ClientViewportObservationSchema } from './workbench-client-viewport-contract';

export const ClientPresentationSelectionSchema = z
	.strictObject({
		presentationId: TerminalPresentationIdSchema,
		binding: WorkbenchPresentationBindingSchema,
		attachmentGeneration: WorkbenchRevisionSchema,
		incarnation: z.number().int().min(0).max(4_294_967_295),
		viewport: ClientViewportObservationSchema,
	})
	.readonly();
export const ClientPresentationSnapshotSchema = z
	.strictObject({
		workbenchRevision: WorkbenchRevisionSchema,
		panelId: WorkbenchPanelIdSchema,
		presentation: ClientPresentationSelectionSchema.nullable(),
	})
	.refine((value) => new TextEncoder().encode(JSON.stringify(value)).byteLength <= 4 * 1024)
	.readonly();
export type ClientPresentationSelection = z.infer<typeof ClientPresentationSelectionSchema>;
export type ClientPresentationSnapshot = z.infer<typeof ClientPresentationSnapshotSchema>;
