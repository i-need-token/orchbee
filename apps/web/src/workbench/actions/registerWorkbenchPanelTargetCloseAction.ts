import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-layout-types';
import { findPanel } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import { terminalCloseImpact, terminalCloseWarningValues } from '../model/terminal-close-impact';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority, WorkbenchAuthorityClosure } from '../types/workbench-authority';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchPanelTargetInput,
	workbenchPanelTargetInput,
} from './workbench-action-ids';

interface RegisterWorkbenchPanelTargetCloseActionOptions {
	readonly authority: WorkbenchAuthority;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

function closureFor(
	input: WorkbenchPanelTargetInput,
	type: 'panel-close' | 'panel-close-if-idle',
): WorkbenchAuthorityClosure {
	return {
		panelId: input.panelId,
		tabId: input.tabId,
		terminalContextId: input.terminalContextId,
		type,
	};
}

/**
 * Closes a Panel named by its location, in whichever Workspace shows it, so a sidebar Agent row can
 * close its Agent without moving the view. As the Panel's own Close, it ends every terminal the
 * Panel holds, and asks first when one of them runs something.
 */
export function registerWorkbenchPanelTargetCloseAction({
	authority,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchPanelTargetCloseActionOptions): readonly ActionDisposable[] {
	const category = { id: 'panels', title: workbenchMessages.actionCategoryPanels };
	const execution = {
		concurrency: 'serialize' as const,
		// Shares the Panel's own close question, so the two closes of one Panel wait for each other.
		concurrencyKey: () => 'panel-close-confirmation',
		mode: 'foreground' as const,
	};
	const resolvePanel = (input: WorkbenchPanelTargetInput) => {
		const context = projectWorkspaceRuntime.getTerminalContext(input.terminalContextId);
		const tab = context?.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === input.tabId);
		return tab === undefined ? null : findPanel(tab.root, input.panelId);
	};
	const closedTerminals = (input: WorkbenchPanelTargetInput): readonly TerminalPresentation[] => {
		const panel = resolvePanel(input);
		return panel === null ? [] : [panel.primaryPresentation, ...panel.childPresentations];
	};
	return [
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution,
				getAvailability: (_context, input) =>
					resolvePanel(input) === null
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				// A Panel that runs nothing closes without asking, and only the authority can tell.
				handler: {
					execute: async (context, input) => {
						const result = await authority.close(context, closureFor(input, 'panel-close-if-idle'));
						if (result.kind !== 'blocked' || result.code !== 'confirmation-required') {
							return result;
						}
						// The question is its own Action, because this one has already asked the authority.
						typedRuntime.execute(WORKBENCH_ACTION_IDS.closePanelTargetConfirmed, input, {
							scope,
							source: context.source,
							target: { id: input.panelId, type: 'panel' },
						});
						return { kind: 'unchanged' };
					},
				},
				id: WORKBENCH_ACTION_IDS.closePanelTarget,
				input: workbenchPanelTargetInput,
				title: workbenchMessages.panelClose,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelClose,
					description: {
						...workbenchMessages.terminalCloseWarning,
						values: terminalCloseWarningValues(terminalCloseImpact(closedTerminals(input))),
					},
					risk: 'destructive',
					title: workbenchMessages.panelCloseTitle,
					watch: (withdraw) => authority.watchClose(closureFor(input, 'panel-close'), withdraw),
				}),
				execution,
				getAvailability: (_context, input) =>
					resolvePanel(input) === null
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) => authority.close(context, closureFor(input, 'panel-close')),
				},
				id: WORKBENCH_ACTION_IDS.closePanelTargetConfirmed,
				input: workbenchPanelTargetInput,
				title: workbenchMessages.panelClose,
			}),
		),
	];
}
