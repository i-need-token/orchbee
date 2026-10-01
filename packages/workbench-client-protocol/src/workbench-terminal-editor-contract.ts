import { z } from 'zod';
import { ClientPresentationSelectionSchema } from './workbench-client-presentation-contract';
import { WorkbenchTerminalGeometrySchema } from './workbench-terminal-launch-contract';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchPanelIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_TERMINAL_EDITOR_BYTES = 8 * 1024;
const encoder = new TextEncoder();
const bounded = (value: unknown) =>
	encoder.encode(JSON.stringify(value)).byteLength <= WORKBENCH_MAXIMUM_TERMINAL_EDITOR_BYTES;
export const ScrollbackEditorSpecSchema = z
	.strictObject({
		source: z.strictObject({
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			member: ClientPresentationSelectionSchema.refine((member) => 'binding' in member),
		}),
	})
	.readonly();
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
const popup = { popupId: WorkbenchRequestIdSchema };
export const TerminalEditorRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.terminal-editor'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('open'),
				...popup,
				spec: ScrollbackEditorSpecSchema,
				capture: z.strictObject({
					attachmentId: WorkbenchRequestIdSchema,
					generation: WorkbenchRevisionSchema.refine((value) => value !== '0'),
					revision: WorkbenchRevisionSchema,
				}),
				geometry: WorkbenchTerminalGeometrySchema,
			}),
			z.strictObject({ kind: z.literal('read'), ...popup }),
			z.strictObject({ kind: z.literal('close'), ...popup }),
		]),
	})
	.refine(bounded)
	.readonly();
export const TerminalEditorResultSchema = z
	.strictObject({
		kind: z.literal('workbench.terminal-editor.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('opened'),
				...popup,
				title: z
					.string()
					.min(1)
					.refine(
						(value) =>
							!/[\uD800-\uDFFF]/u.test(value) &&
							!value.includes('\0') &&
							encoder.encode(value).byteLength <= 4096,
					),
			}),
			z.strictObject({ kind: z.literal('state'), ...popup, settled: z.boolean() }),
			z.strictObject({ kind: z.literal('closing'), ...popup }),
			z.strictObject({ kind: z.literal('closed'), ...popup }),
			z.strictObject({ kind: z.literal('unknown'), ...popup }),
			z.strictObject({
				kind: z.literal('failed'),
				reason: z.enum([
					'denied',
					'invalid-intent',
					'site-mismatch',
					'unavailable',
					'busy',
					'conflict',
					'not-found',
					'stale-source',
					'editor-unavailable',
					'physical-unknown',
				]),
			}),
		]),
	})
	.refine(bounded)
	.readonly();
export type ScrollbackEditorSpec = z.infer<typeof ScrollbackEditorSpecSchema>;
export type TerminalEditorRequest = z.infer<typeof TerminalEditorRequestSchema>;
export type TerminalEditorResult = z.infer<typeof TerminalEditorResultSchema>;
