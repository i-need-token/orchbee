import { z } from 'zod';
import { ClientPresentationSelectionSchema } from './workbench-client-presentation-contract';
import { PluginInvocationContextSchema } from './workbench-plugin-context-contract';
import {
	WorkbenchLaunchEnvironmentSchema,
	WorkbenchPluginPaneSelectorSchema,
} from './workbench-terminal-launch-contract';
import {
	WorkbenchDirectoryPathSchema,
	WorkbenchPanelIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from './workbench-semantic-contract';

const PluginPopupSizeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cells'), value: z.number().int().min(0).max(65535) }),
	z.strictObject({ kind: z.literal('percent'), value: z.number().int().min(1).max(100) }),
]);
export const PluginPopupSpecSchema = z
	.strictObject({
		selector: WorkbenchPluginPaneSelectorSchema,
		source: z.strictObject({
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			member: ClientPresentationSelectionSchema,
		}),
		context: PluginInvocationContextSchema,
		cwd: WorkbenchDirectoryPathSchema.nullable(),
		environment: WorkbenchLaunchEnvironmentSchema,
		width: PluginPopupSizeSchema.nullable(),
		height: PluginPopupSizeSchema.nullable(),
	})
	.refine(
		(spec) =>
			spec.context.tabId === spec.source.tabId &&
			spec.context.focusedPanelId === spec.source.panelId,
		'Popup invocation context must match its observed source',
	)
	.refine(
		(spec) => new TextEncoder().encode(JSON.stringify(spec.source.member)).byteLength <= 4 * 1024,
	)
	.readonly();
