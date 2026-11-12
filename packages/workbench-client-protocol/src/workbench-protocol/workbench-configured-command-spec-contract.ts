import { z } from 'zod';
import { ClientPresentationSelectionSchema } from './workbench-client-presentation-contract';
import { PluginInvocationContextSchema } from './workbench-plugin-context-contract';
import {
	WorkbenchPanelIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';

const bytes = (value: string) => new TextEncoder().encode(value).byteLength;
const hasControlCharacter = (value: string) =>
	[...value].some((character) => {
		const code = character.charCodeAt(0);
		return code < 32 || (code >= 127 && code <= 159);
	});
export const ConfiguredCommandIdSchema = z
	.string()
	.max(120)
	.regex(/^cmd_[A-Za-z0-9_]*$/u);
const ConfiguredCommandSizeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cells'), value: z.number().int().min(1).max(4096) }),
	z.strictObject({ kind: z.literal('percent'), value: z.number().int().min(1).max(100) }),
]);
export const ConfiguredCommandDescriptorSchema = z
	.strictObject({
		id: ConfiguredCommandIdSchema,
		action: z.enum(['shell', 'panel', 'popup', 'plugin-action']),
		bindings: z
			.array(
				z
					.string()
					.min(1)
					.refine((value) => bytes(value) <= 128 && !hasControlCharacter(value)),
			)
			.max(16),
		description: z
			.string()
			.refine(
				(value) =>
					value.trim().length > 0 &&
					!value.includes('\0') &&
					!/[\uD800-\uDFFF]/u.test(value) &&
					bytes(value) <= 512,
			)
			.nullable(),
		// The command line or plugin target as the TUI's key help shows it: display text only.
		display: z
			.string()
			.refine(
				(value) =>
					value.trim().length > 0 &&
					!hasControlCharacter(value) &&
					!/[\uD800-\uDFFF]/u.test(value) &&
					bytes(value) <= 512,
			)
			.nullable(),
		width: ConfiguredCommandSizeSchema.nullable(),
		height: ConfiguredCommandSizeSchema.nullable(),
	})
	.refine((value) => value.action === 'popup' || (value.width === null && value.height === null))
	.readonly();
export const ConfiguredCommandPopupSourceSchema = z.strictObject({
	terminalContextId: WorkbenchTerminalContextIdSchema,
	tabId: WorkbenchTabIdSchema,
	panelId: WorkbenchPanelIdSchema,
	member: ClientPresentationSelectionSchema.refine((member) => 'binding' in member),
});
export const ConfiguredCommandPopupSpecSchema = z
	.strictObject({
		commandId: ConfiguredCommandIdSchema,
		source: ConfiguredCommandPopupSourceSchema,
		context: PluginInvocationContextSchema,
		width: ConfiguredCommandSizeSchema.nullable(),
		height: ConfiguredCommandSizeSchema.nullable(),
	})
	.refine(
		(value) =>
			value.context.tabId === value.source.tabId &&
			value.context.focusedPanelId === value.source.panelId,
	)
	.readonly();
const point = z.strictObject({
	row: WorkbenchRevisionSchema,
	col: z.number().int().min(0).max(4095),
});
export const ConfiguredCommandSelectionSchema = z
	.strictObject({ anchor: point, cursor: point, revision: WorkbenchRevisionSchema })
	.refine((value) => {
		const difference = BigInt(value.anchor.row) - BigInt(value.cursor.row);
		return difference > -4096n && difference < 4096n;
	})
	.readonly();
