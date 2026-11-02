import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';

import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTabTitleInput,
	workbenchTabTitleInput,
} from './workbench-action-ids';

export interface WorkbenchTabTitleEditorRequest {
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly title: string;
	/**
	 * The shared name follows the terminal until someone names it, so there is nothing to edit:
	 * `title` is only what shows now, and only what the person types becomes a name.
	 */
	readonly unnamed: boolean;
}

interface RegisterWorkbenchTabTitleActionOptions {
	readonly authority?: WorkbenchAuthority | undefined;
	readonly openEditor: (request: WorkbenchTabTitleEditorRequest) => void;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

export function registerWorkbenchTabTitleAction({
	authority,
	openEditor,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchTabTitleActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: {
				id: 'tabs',
				title: workbenchTabMessages.actionCategoryTabs,
			},
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => targetKey(input),
				mode: 'foreground',
			},
			getAvailability: (_context, input) =>
				resolveTarget(projectWorkspaceRuntime, input) === null
					? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
					: { kind: 'enabled' },
			handler: {
				execute: (context, input) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					const target = resolveTarget(projectWorkspaceRuntime, input);
					if (target === null) {
						return { code: 'tab-title-target-unavailable', kind: 'blocked' };
					}
					const titlePolicy = target.terminalContext.panelWorkspaceStore
						.getTabTitlePolicies()
						.find((policy) => policy.tabId === target.tabId);
					if (input?.kind !== 'commit') {
						context.markCommitted();
						openEditor({
							tabId: target.tabId,
							terminalContextId: target.terminalContext.terminalContextId,
							title: target.title,
							unnamed: authority !== undefined && titlePolicy?.customTitle === null,
						});
						return { kind: 'completed' };
					}

					if (authority !== undefined) {
						const label = input.title?.trim() ?? '';
						// A shared name has no way back to following the terminal, so empty asks for nothing.
						return label === ''
							? { kind: 'unchanged' }
							: authority.change(context, {
									label,
									tabId: target.tabId,
									terminalContextId: target.terminalContext.terminalContextId,
									type: 'tab-renamed',
								});
					}
					const title = input.title?.trim() || null;
					if (titlePolicy === undefined) {
						return { code: 'tab-title-target-unavailable', kind: 'blocked' };
					}
					if (titlePolicy.customTitle === title) return { kind: 'unchanged' };

					context.markCommitted();
					const result = target.terminalContext.panelWorkspaceStore.setTabCustomTitle(
						target.tabId,
						title,
					);
					if (result.kind === 'rejected') {
						throw new Error(
							'The validated Tab title target disappeared during synchronous commit.',
						);
					}
					return result.kind === 'unchanged' ? { kind: 'unchanged' } : { kind: 'completed' };
				},
			},
			id: WORKBENCH_ACTION_IDS.setTabTitle,
			input: workbenchTabTitleInput,
			title: workbenchTabMessages.setTabTitle,
		}),
	);
}

function resolveTarget(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchTabTitleInput,
): {
	readonly tabId: string;
	readonly terminalContext: WorkbenchTerminalContext;
	readonly title: string;
} | null {
	const terminalContext =
		input === undefined
			? projectWorkspaceRuntime.getActiveTerminalContext()
			: projectWorkspaceRuntime.getTerminalContext(input.terminalContextId);
	if (terminalContext === null) return null;
	const tabId =
		input === undefined ? terminalContext.tabModel.getSnapshot().activeTabId : input.tabId;
	if (tabId === null) return null;
	const tab = terminalContext.tabModel
		.getSnapshot()
		.tabs.find((candidate) => candidate.id === tabId);
	return tab === undefined ? null : { tabId, terminalContext, title: tab.title };
}

function targetKey(input: WorkbenchTabTitleInput): string {
	return input === undefined ? 'active-tab' : `${input.terminalContextId}:${input.tabId}`;
}
