import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';

import type { TerminalCloseImpact } from '../model/terminal-close-impact';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority, WorkbenchTabCloseStop } from '../types/workbench-authority';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import { closeReturningToTerminal } from './closeReturningToTerminal';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTabTargetInput,
	workbenchTabTargetInput,
} from './workbench-action-ids';
import type { WorkbenchFocusAfterTabClose } from './workbench-tab-close-focus';

interface RegisterWorkbenchTabCloseResumeActionOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: WorkbenchFocusRouting['beginTerminalHandover'];
	readonly focusAfterTabClose: WorkbenchFocusAfterTabClose;
	/** The localized sentence naming what a close ends; keep it stable across renders. */
	readonly formatCloseWarning: (impact: TerminalCloseImpact) => string;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

interface StoppedTab {
	readonly stop: WorkbenchTabCloseStop;
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly title: string;
}

/**
 * Resume closing this Tab, as the TUI offers it: only for a Tab of the shown Workspace whose close
 * stopped where it may continue, always after asking, and never as another close in its place.
 */
export function registerWorkbenchTabCloseResumeAction({
	authority,
	beginTerminalHandover,
	focusAfterTabClose,
	formatCloseWarning,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchTabCloseResumeActionOptions): ActionDisposable {
	const stoppedTab = (input: WorkbenchTabTargetInput): StoppedTab | null => {
		const active = projectWorkspaceRuntime.getActiveTerminalContext();
		if (active === null) return null;
		const { tabModel, terminalContextId } = active;
		if (input !== undefined && input.terminalContextId !== terminalContextId) return null;
		const snapshot = tabModel.getSnapshot();
		const tabId = input?.tabId ?? snapshot.activeTabId;
		const tab = snapshot.tabs.find((candidate) => candidate.id === tabId);
		if (tab === undefined) return null;
		const stop = authority.stoppedTabClose(terminalContextId, tab.id);
		return stop === null ? null : { stop, tabId: tab.id, terminalContextId, title: tab.title };
	};
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'tabs', title: workbenchTabMessages.actionCategoryTabs },
			confirmation: (input) => {
				const stopped = stoppedTab(input);
				if (stopped === null) return null;
				const impact = projectWorkspaceRuntime.getActiveTerminalTabsCloseImpact([stopped.tabId]);
				const closure = {
					tabId: stopped.tabId,
					terminalContextId: stopped.terminalContextId,
					type: 'tab-close-resume' as const,
				};
				return {
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelClose,
					defaultAnswer: 'confirm',
					description: {
						...workbenchMessages.tabCloseResumeDescription,
						values: {
							label: stopped.title,
							stop: stopped.stop,
							// The Server reported every terminal of a Tab awaiting removal as disposed.
							warning: formatCloseWarning(
								stopped.stop === 'awaiting-removal'
									? { ended: impact.ended + impact.running, running: 0 }
									: impact,
							),
						},
					},
					risk: 'destructive',
					title: workbenchMessages.tabCloseResumeTitle,
					watch: (withdraw) => authority.watchClose(closure, withdraw),
				};
			},
			execution: {
				concurrency: 'serialize',
				// The Close key names no Tab, so it shares the key of the Tab it means.
				concurrencyKey: (input) => {
					const active = projectWorkspaceRuntime.getActiveTerminalContext();
					const tabId = input?.tabId ?? active?.tabModel.getSnapshot().activeTabId;
					return `${input?.terminalContextId ?? active?.terminalContextId ?? ''}:${tabId ?? ''}`;
				},
				mode: 'foreground',
			},
			// Only a close that stopped where it may continue offers this, so Search shows it only then.
			getAvailability: (_context, input) =>
				stoppedTab(input) === null ? { kind: 'hidden' } : { kind: 'enabled' },
			handler: {
				execute: async (context, input) => {
					const stopped = stoppedTab(input);
					if (stopped === null) return { code: 'scope-changed', kind: 'blocked' };
					return closeReturningToTerminal(
						authority,
						context,
						{
							tabId: stopped.tabId,
							terminalContextId: stopped.terminalContextId,
							type: 'tab-close-resume',
						},
						beginTerminalHandover(),
						focusAfterTabClose,
					);
				},
			},
			id: WORKBENCH_ACTION_IDS.resumeTabClose,
			input: workbenchTabTargetInput,
			title: workbenchMessages.tabCloseResume,
		}),
	);
}
