import type { ActionAvailability, ActionExecutionContext, ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import type { TabStripTabCreationIntent } from '@orchbee/tab-strip';

import type { WorkbenchTerminalCreateInput } from './workbench-action-ids';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityTabCreation,
	WorkbenchTabDuplicationStop,
} from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import type { WorkbenchTerminalCreateRequest } from '../types/workbench-host-actions';

interface SharedTabCreationOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

interface SharedTabDuplicationOptions {
	readonly authority: WorkbenchAuthority;
	readonly onStopped: (
		stop: WorkbenchTabDuplicationStop,
		counts: { readonly created: number; readonly total: number },
	) => void;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

const unavailable: ActionAvailability = {
	kind: 'disabled',
	reason: workbenchMessages.actionUnavailable,
};

const ACTIVE_TAB_UNAVAILABLE = { code: 'active-tab-unavailable', kind: 'blocked' } as const;

/**
 * Without a Workspace, only a plain New Tab means anything: as the TUI's does, it starts a Workspace,
 * while a Tab placed in a group or at a position waits for one.
 */
export function getSharedTabCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	intent: TabStripTabCreationIntent,
): ActionAvailability {
	const terminalContext = runtime.getActiveTerminalContext();
	if (terminalContext === null) {
		return intent.kind === 'new-tab' && intent.groupId === null && intent.index === undefined
			? { kind: 'enabled' }
			: unavailable;
	}
	if (intent.kind !== 'new-tab' || intent.index === undefined) return { kind: 'enabled' };
	// A new Tab is never pinned, so the authority refuses one placed among the pinned Tabs.
	const { tabs } = terminalContext.tabModel.getSnapshot();
	return intent.index < tabs.filter((tab) => tab.pinned).length ? unavailable : { kind: 'enabled' };
}

/** A launch profile only chooses how a Tab starts, so it never stands in for the missing Workspace. */
export function getSharedTerminalCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchTerminalCreateInput,
): ActionAvailability {
	if (input.profile !== undefined && runtime.getActiveTerminalContext() === null) {
		return unavailable;
	}
	return getSharedTabCreationAvailability(runtime, {
		groupId: input.groupId,
		kind: 'new-tab',
		...(input.index === undefined ? {} : { index: input.index }),
	});
}

function withoutReceipt<Receipt>(result: ActionResult<Receipt>): ActionResult<void> {
	return result.kind === 'completed' || result.kind === 'unchanged'
		? { kind: result.kind }
		: result;
}

/**
 * Selects the new Tab and gives its terminal the keyboard, unless the person has moved on while the
 * authority decided: late completion never takes focus or navigation from later work.
 */
export async function createSharedTab(
	context: ActionExecutionContext,
	creation: Omit<WorkbenchAuthorityTabCreation, 'terminalContextId'>,
	{ authority, beginTerminalHandover, runtime }: SharedTabCreationOptions,
): Promise<ActionResult<void>> {
	const origin = runtime.getSnapshot().activeLocation;
	const originContext = runtime.getActiveTerminalContext();
	if (originContext === null) return ACTIVE_TAB_UNAVAILABLE;
	const handover = beginTerminalHandover();
	const created = await authority.createTab(context, {
		...creation,
		terminalContextId: originContext.terminalContextId,
	});
	if (created.kind !== 'completed' || created.value === undefined) return withoutReceipt(created);
	const { tabId, terminalContextId } = created.value;
	const tabModel = runtime.getTerminalContext(terminalContextId)?.tabModel;
	if (
		tabModel?.getSnapshot().tabs.some((tab) => tab.id === tabId) === true &&
		!context.signal.aborted &&
		handover.held() &&
		workbenchActiveLocationsEqual(origin, runtime.getSnapshot().activeLocation)
	) {
		tabModel.selectTab(tabId);
		handover.complete();
	}
	return { kind: 'completed' };
}

/** New Tab, New Tab to right, New Tab in Group, and New Tab in the current directory. */
export function createSharedTerminal(
	context: ActionExecutionContext,
	input: WorkbenchTerminalCreateRequest,
	options: SharedTabCreationOptions,
): ActionResult<void> | Promise<ActionResult<void>> {
	if (context.signal.aborted) return { kind: 'cancelled' };
	const launch = input.profile === undefined ? {} : { profile: input.profile };
	if (input.currentDirectoryTarget !== undefined) {
		const directory = options.runtime.resolveTerminalCurrentDirectoryTarget(
			input.currentDirectoryTarget,
		);
		return directory.kind === 'ready'
			? createSharedTab(context, { cwd: directory.cwd, placement: null, ...launch }, options)
			: { code: directory.reason, kind: 'blocked' };
	}
	const tabs = options.runtime.getActiveTerminalContext()?.tabModel.getSnapshot().tabs ?? [];
	const previous = input.index === undefined ? undefined : tabs[input.index - 1];
	if (previous !== undefined) {
		return createSharedTab(
			context,
			{ placement: { kind: 'after-tab', tabId: previous.id }, ...launch },
			options,
		);
	}
	return createSharedTab(
		context,
		{
			placement: input.groupId === null ? null : { groupId: input.groupId, kind: 'in-group' },
			...launch,
		},
		options,
	);
}

/**
 * As in the TUI, one Tab is an ordinary creation and several are one operation that copies them in
 * the order the strip shows them, each right after itself. The person stays where they are. Anything
 * uncertain stops the rest, because it is never repeated.
 */
export async function duplicateSharedTabs(
	context: ActionExecutionContext,
	tabIds: readonly string[],
	{ authority, onStopped, runtime }: SharedTabDuplicationOptions,
): Promise<ActionResult<void>> {
	const active = runtime.getActiveTerminalContext();
	if (active === null) return ACTIVE_TAB_UNAVAILABLE;
	const { tabModel, terminalContextId } = active;
	const requested = new Set(tabIds);
	const sources = tabModel
		.getSnapshot()
		.tabs.filter((tab) => requested.has(tab.id))
		.map((tab) => tab.id);
	const stopped = (
		stop: WorkbenchTabDuplicationStop,
		created: number,
		total: number,
	): ActionResult<void> => {
		onStopped(stop, { created, total });
		return { failure: { code: `duplication-${stop}` }, kind: 'failed' };
	};
	const [source] = sources;
	if (source === undefined) return { kind: 'completed' };
	if (sources.length === 1) {
		const copy = await authority.createTab(context, {
			placement: { kind: 'duplicate', tabId: source },
			terminalContextId,
		});
		if (copy.kind === 'cancelled') return copy;
		if (copy.kind !== 'completed') {
			return copy.kind === 'failed' && copy.failure.code === 'outcome-unknown'
				? stopped('terminal-not-started', 0, 1)
				: stopped('not-created', 0, 1);
		}
		return copy.value?.terminalStarted === true
			? { kind: 'completed' }
			: stopped('terminal-not-started', 1, 1);
	}
	const duplicated = await authority.duplicateTabs(context, { tabIds: sources, terminalContextId });
	if (duplicated.kind !== 'completed') return withoutReceipt(duplicated);
	const report = duplicated.value;
	return report === undefined || report.kind === 'duplicated'
		? { kind: 'completed' }
		: stopped(report.stop, report.created, report.total);
}
