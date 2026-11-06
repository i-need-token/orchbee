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
	type WorkbenchTabSetTargetInput,
	workbenchTabSetTargetInput,
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

/** The TUI names this many Tabs in a close question and counts the rest. */
const NAMED_TABS = 3;

interface ShownTabs {
	readonly tabIds: readonly string[];
	readonly terminalContextId: string;
	readonly titles: readonly string[];
}

function closureFor({ tabIds, terminalContextId }: ShownTabs) {
	return { tabIds, terminalContextId, type: 'tabs-close' as const };
}

/**
 * Close these Tabs after asking. A Close of one Tab or of a set asks the authority first, as the
 * TUI does, and raises this question only when the authority answers that something may still run
 * in one of them.
 */
export function registerWorkbenchTabCloseConfirmedAction({
	authority,
	beginTerminalHandover,
	formatCloseWarning,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchTabCloseConfirmedActionOptions): ActionDisposable {
	/** The Tabs in the order the strip shows them, while it still shows every one. */
	const shownTabs = (input: WorkbenchTabSetTargetInput): ShownTabs | null => {
		const active = projectWorkspaceRuntime.getActiveTerminalContext();
		if (input === undefined || active?.terminalContextId !== input.terminalContextId) return null;
		const requested = new Set(input.tabIds);
		const tabs = active.tabModel.getSnapshot().tabs.filter((tab) => requested.has(tab.id));
		return tabs.length === requested.size
			? {
					tabIds: tabs.map((tab) => tab.id),
					terminalContextId: input.terminalContextId,
					titles: tabs.map((tab) => tab.title),
				}
			: null;
	};
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'tabs', title: workbenchTabMessages.actionCategoryTabs },
			confirmation: (input) => {
				const tabs = shownTabs(input);
				if (tabs === null) return null;
				const named = tabs.titles.slice(0, NAMED_TABS);
				return {
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelClose,
					defaultAnswer: 'confirm',
					description: {
						...workbenchMessages.tabCloseDescription,
						values: {
							more: tabs.titles.length - named.length,
							names: named.join(', '),
							warning: formatCloseWarning(
								projectWorkspaceRuntime.getActiveTerminalTabsCloseImpact(tabs.tabIds),
							),
						},
					},
					risk: 'destructive',
					title: { ...workbenchMessages.tabCloseTitle, values: { count: tabs.tabIds.length } },
					watch: (withdraw) => authority.watchClose(closureFor(tabs), withdraw),
				};
			},
			execution: {
				concurrency: 'serialize',
				concurrencyKey: (input) =>
					`${input?.terminalContextId ?? ''}:${input?.tabIds.join(',') ?? ''}`,
				mode: 'foreground',
			},
			// It names the Tabs the authority asked about, so without them nothing offers it.
			getAvailability: (_context, input) =>
				shownTabs(input) === null ? { kind: 'hidden' } : { kind: 'enabled' },
			handler: {
				execute: async (context, input) => {
					const tabs = shownTabs(input);
					if (tabs === null) return { code: 'scope-changed', kind: 'blocked' };
					const handover = beginTerminalHandover();
					const result = await authority.close(context, closureFor(tabs));
					if (result.kind === 'completed') handover.complete();
					return result;
				},
			},
			id: WORKBENCH_ACTION_IDS.closeTabConfirmed,
			input: workbenchTabSetTargetInput,
			title: workbenchTabMessages.close,
		}),
	);
}
