import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';

import type { TerminalCloseImpact } from '../model/terminal-close-impact';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTabTargetInput,
	workbenchTabTargetInput,
} from './workbench-action-ids';

interface RegisterWorkbenchTabCloseConfirmedActionOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: WorkbenchFocusRouting['beginTerminalHandover'];
	/** The localized sentence naming what a close ends; keep it stable across renders. */
	readonly formatCloseWarning: (impact: TerminalCloseImpact) => string;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

interface ShownTab {
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly title: string;
}

/**
 * Close this Tab after asking. One Tab's Close asks the authority first, as the TUI does, and raises
 * this question only when the authority answers that something may still run in it.
 */
export function registerWorkbenchTabCloseConfirmedAction({
	authority,
	beginTerminalHandover,
	formatCloseWarning,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchTabCloseConfirmedActionOptions): ActionDisposable {
	const shownTab = (input: WorkbenchTabTargetInput): ShownTab | null => {
		const active = projectWorkspaceRuntime.getActiveTerminalContext();
		if (input === undefined || active?.terminalContextId !== input.terminalContextId) return null;
		const tab = active.tabModel
			.getSnapshot()
			.tabs.find((candidate) => candidate.id === input.tabId);
		return tab === undefined
			? null
			: { tabId: tab.id, terminalContextId: input.terminalContextId, title: tab.title };
	};
	const closureFor = ({ tabId, terminalContextId }: ShownTab) => ({
		tabIds: [tabId],
		terminalContextId,
		type: 'tabs-close' as const,
	});
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'tabs', title: workbenchTabMessages.actionCategoryTabs },
			confirmation: (input) => {
				const tab = shownTab(input);
				if (tab === null) return null;
				return {
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelClose,
					description: {
						...workbenchMessages.tabCloseDescription,
						values: {
							more: 0,
							names: tab.title,
							warning: formatCloseWarning(
								projectWorkspaceRuntime.getActiveTerminalTabsCloseImpact([tab.tabId]),
							),
						},
					},
					risk: 'destructive',
					title: { ...workbenchMessages.tabCloseTitle, values: { count: 1 } },
					watch: (withdraw) => authority.watchClose(closureFor(tab), withdraw),
				};
			},
			execution: {
				concurrency: 'serialize',
				concurrencyKey: (input) => `${input?.terminalContextId ?? ''}:${input?.tabId ?? ''}`,
				mode: 'foreground',
			},
			// It names the Tab the authority asked about, so without one nothing offers it.
			getAvailability: (_context, input) =>
				shownTab(input) === null ? { kind: 'hidden' } : { kind: 'enabled' },
			handler: {
				execute: async (context, input) => {
					const tab = shownTab(input);
					if (tab === null) return { code: 'scope-changed', kind: 'blocked' };
					const handover = beginTerminalHandover();
					const result = await authority.close(context, closureFor(tab));
					if (result.kind === 'completed') handover.complete();
					return result;
				},
			},
			id: WORKBENCH_ACTION_IDS.closeTabConfirmed,
			input: workbenchTabTargetInput,
			title: workbenchTabMessages.close,
		}),
	);
}
