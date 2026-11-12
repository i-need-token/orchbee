import { z } from 'zod';
import { WorkerIdSchema, WorkbenchDirectoryPathSchema } from './workbench-semantic-contract';

const bytes = (value: string) => new TextEncoder().encode(value).length;
const text = (maximum: number, nul = false) =>
	z
		.string()
		.refine(
			(value) =>
				!/[\uD800-\uDFFF]/u.test(value) &&
				(nul || !value.includes('\0')) &&
				bytes(value) <= maximum,
		);
const optionalText = text(4096).nullable().optional();
const worktreeContext = z.strictObject({
	workerId: WorkerIdSchema,
	repositoryRoot: WorkbenchDirectoryPathSchema,
	commonDirectory: WorkbenchDirectoryPathSchema,
	repositoryName: text(4096).refine((value) => value.length > 0),
	checkoutPath: WorkbenchDirectoryPathSchema,
	linked: z.boolean(),
});
export const PluginInvocationContextSchema = z
	.strictObject({
		workspaceId: optionalText,
		workspaceLabel: optionalText,
		workspaceCwd: optionalText,
		worktree: worktreeContext.nullable().optional(),
		tabId: optionalText,
		tabLabel: optionalText,
		focusedPanelId: optionalText,
		focusedPanelCwd: optionalText,
		focusedPanelAgent: optionalText,
		focusedPanelStatus: optionalText,
		selectedText: text(8192).nullable().optional(),
		invocationSource: optionalText,
		correlationId: optionalText,
		clickedUrl: optionalText,
		linkHandlerId: optionalText,
	})
	.refine(
		(value) =>
			bytes(
				JSON.stringify(
					Object.fromEntries(
						Object.entries(value).filter(([, item]) => item !== null && item !== undefined),
					),
				),
			) <= 16384,
	);
/** Drop optional nulls as the native context serializer does. */
export function normalizePluginInvocationContext(
	value: z.output<typeof PluginInvocationContextSchema>,
): z.output<typeof PluginInvocationContextSchema> {
	return {
		...(value.workspaceId !== null && value.workspaceId !== undefined
			? { workspaceId: value.workspaceId }
			: {}),
		...(value.workspaceLabel !== null && value.workspaceLabel !== undefined
			? { workspaceLabel: value.workspaceLabel }
			: {}),
		...(value.workspaceCwd !== null && value.workspaceCwd !== undefined
			? { workspaceCwd: value.workspaceCwd }
			: {}),
		...(value.worktree !== null && value.worktree !== undefined
			? { worktree: value.worktree }
			: {}),
		...(value.tabId !== null && value.tabId !== undefined ? { tabId: value.tabId } : {}),
		...(value.tabLabel !== null && value.tabLabel !== undefined
			? { tabLabel: value.tabLabel }
			: {}),
		...(value.focusedPanelId !== null && value.focusedPanelId !== undefined
			? { focusedPanelId: value.focusedPanelId }
			: {}),
		...(value.focusedPanelCwd !== null && value.focusedPanelCwd !== undefined
			? { focusedPanelCwd: value.focusedPanelCwd }
			: {}),
		...(value.focusedPanelAgent !== null && value.focusedPanelAgent !== undefined
			? { focusedPanelAgent: value.focusedPanelAgent }
			: {}),
		...(value.focusedPanelStatus !== null && value.focusedPanelStatus !== undefined
			? { focusedPanelStatus: value.focusedPanelStatus }
			: {}),
		...(value.selectedText !== null && value.selectedText !== undefined
			? { selectedText: value.selectedText }
			: {}),
		...(value.invocationSource !== null && value.invocationSource !== undefined
			? { invocationSource: value.invocationSource }
			: {}),
		...(value.correlationId !== null && value.correlationId !== undefined
			? { correlationId: value.correlationId }
			: {}),
		...(value.clickedUrl !== null && value.clickedUrl !== undefined
			? { clickedUrl: value.clickedUrl }
			: {}),
		...(value.linkHandlerId !== null && value.linkHandlerId !== undefined
			? { linkHandlerId: value.linkHandlerId }
			: {}),
	};
}
