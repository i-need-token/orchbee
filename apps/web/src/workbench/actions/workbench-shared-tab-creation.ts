import type { ActionAvailability, ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { TabStripTabCreationIntent } from '@orchbee/tab-strip';

import { findWorkbenchWorkspace } from '../../project-workspace';
import type { WorkbenchTerminalCreateInput } from './workbench-action-ids';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityTabCreation,
	WorkbenchTabDuplicationStop,
} from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import type {
	WorkbenchNavigationPort,
	WorkbenchTerminalCreateRequest,
} from '../types/workbench-host-actions';

interface SharedTabCreationOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
	readonly workspaceId?: string | undefined;
}

interface SharedTabDuplicationOptions {
	readonly authority: WorkbenchAuthority;
	readonly onStopped: (stop: WorkbenchTabDuplicationStop) => void;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

const WORKSPACE_REQUIRED: ActionAvailability = {
	kind: 'disabled',
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
			: WORKSPACE_REQUIRED;
	}
	if (intent.kind !== 'new-tab' || intent.index === undefined) return { kind: 'enabled' };
	// A new Tab is never pinned, so the authority refuses one placed among the pinned Tabs.
	const { tabs } = terminalContext.tabModel.getSnapshot();
	return intent.index < tabs.filter((tab) => tab.pinned).length
		? { kind: 'disabled' }
		: { kind: 'enabled' };
}

/** A launch profile only chooses how a Tab starts, so it never stands in for the missing Workspace. */
export function getSharedTerminalCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchTerminalCreateInput,
): ActionAvailability {
	if (input.profile !== undefined && runtime.getActiveTerminalContext() === null) {
		return WORKSPACE_REQUIRED;
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
	{ authority, beginTerminalHandover, navigation, runtime, workspaceId }: SharedTabCreationOptions,
): Promise<ActionResult<void>> {
	const state = runtime.getSnapshot();
	const origin = state.activeLocation;
	const workspace =
		workspaceId === undefined ? undefined : findWorkbenchWorkspace(state, workspaceId);
	if (workspaceId !== undefined && workspace === undefined) return ACTIVE_TAB_UNAVAILABLE;
	const target =
		workspace === undefined
			? runtime.getActiveTerminalContext()
			: runtime.getTerminalContext(workspace.terminalContextId);
	if (target === null) return ACTIVE_TAB_UNAVAILABLE;
	const handover = beginTerminalHandover();
	const created = await authority.createTab(context, {
		...creation,
		terminalContextId: target.terminalContextId,
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
		if (
			workspace !== undefined &&
			(origin.kind !== 'workspace' || origin.workspaceId !== workspace.id)
		) {
			if (navigation === undefined) {
				const activated = runtime.transition({
					type: 'workspace-activated',
					workspaceId: workspace.id,
				});
				if (activated.outcome === 'rejected') return { kind: 'completed' };
			} else {
				const entered = await navigation.navigate(context, {
					kind: 'workspace',
					workspaceId: workspace.id,
				});
				if (entered.kind !== 'completed' && entered.kind !== 'unchanged')
					return { kind: 'completed' };
			}
		}
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
	const stopped = (stop: WorkbenchTabDuplicationStop): ActionResult<void> => {
		onStopped(stop);
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
				? stopped('terminal-not-started')
				: stopped('not-created');
		}
		return copy.value?.terminalStarted === true
			? { kind: 'completed' }
			: stopped('terminal-not-started');
	}
	const duplicated = await authority.duplicateTabs(context, { tabIds: sources, terminalContextId });
	if (duplicated.kind !== 'completed') return withoutReceipt(duplicated);
	const report = duplicated.value;
	return report === undefined || report.kind === 'duplicated'
		? { kind: 'completed' }
		: stopped(report.stop);
}
