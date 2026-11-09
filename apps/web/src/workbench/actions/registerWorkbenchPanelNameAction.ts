import {
	defineAction,
	type ActionDisposable,
	type ActionMessageDescriptor,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';

import {
	findPanel,
	getTerminalAutomaticTitle,
	getTerminalDisplayTitle,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchTabTitleEditorRequest } from './registerWorkbenchTabTitleAction';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchPanelNameInput,
	workbenchPanelNameInput,
} from './workbench-action-ids';

export interface WorkbenchPanelNameEditorRequest extends WorkbenchTabTitleEditorRequest {
	readonly panelId: string;
	readonly presentationId: string;
	readonly fallbackTitle: string;
}

interface RegisterWorkbenchPanelNameActionOptions {
	readonly authority: WorkbenchAuthority;
	readonly openEditor: (request: WorkbenchPanelNameEditorRequest) => void;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/** The Panel always names its primary Presentation, even while a child terminal is displayed. */
export function registerWorkbenchPanelNameAction({
	authority,
	openEditor,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchPanelNameActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			id: WORKBENCH_ACTION_IDS.renamePanel,
			category: { id: 'panels', title: workbenchMessages.actionCategoryPanels },
			input: workbenchPanelNameInput,
			title: workbenchMessages.panelRenameTitle,
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) =>
					input === undefined
						? 'active-panel-name'
						: `${input.terminalContextId}:${input.tabId}:${input.panelId}`,
				mode: 'foreground',
			},
			getAvailability: (_context, input) => {
				const target = resolve(input);
				return 'unavailable' in target
					? { kind: 'disabled', reason: target.unavailable }
					: { kind: 'enabled' };
			},
			handler: {
				execute: (context, input) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					const target = resolve(input);
					if ('unavailable' in target) {
						return { kind: 'blocked', code: 'panel-name-target-unavailable' };
					}
					if (input?.kind === 'commit')
						return authority.change(context, {
							type: 'panel-renamed',
							terminalContextId: target.terminalContextId,
							tabId: target.tabId,
							panelId: target.panelId,
							label: input.title,
						});
					context.markCommitted();
					openEditor(target);
					return { kind: 'completed' };
				},
			},
		}),
	);

	/** The Panel to name, or why there is none: a named Panel is gone, or the active one is missing. */
	function resolve(
		input: WorkbenchPanelNameInput,
	): WorkbenchPanelNameEditorRequest | { readonly unavailable: ActionMessageDescriptor } {
		const missing = { unavailable: workbenchMessages.panelUnavailableMissing };
		const terminalContext =
			input === undefined
				? projectWorkspaceRuntime.getActiveTerminalContext()
				: projectWorkspaceRuntime.getTerminalContext(input.terminalContextId);
		if (terminalContext === null) {
			return input === undefined
				? { unavailable: workbenchMessages.workspaceUnavailableRequired }
				: missing;
		}
		const store = terminalContext.panelWorkspaceStore;
		const tabId = input?.tabId ?? store.getActiveTabId();
		const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) {
			return input === undefined
				? { unavailable: workbenchTabMessages.unavailableNoneSelected }
				: missing;
		}
		const panel = findPanel(tab.root, input?.panelId ?? tab.focusedPanelId);
		if (panel === null) {
			return input === undefined
				? { unavailable: workbenchMessages.panelUnavailableFocusedPanelRequired }
				: missing;
		}
		const primary = panel.primaryPresentation;
		// The name belongs to the primary terminal the editor opened on; another one is not it.
		if (input?.kind === 'commit' && input.presentationId !== primary.presentationId) return missing;
		return {
			panelId: panel.id,
			tabId: tab.tabId,
			terminalContextId: terminalContext.terminalContextId,
			presentationId: primary.presentationId,
			fallbackTitle: getTerminalAutomaticTitle(primary),
			title: getTerminalDisplayTitle(primary),
			unnamed: primary.customTitle === undefined,
		};
	}
}
