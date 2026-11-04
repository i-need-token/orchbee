import type { ActionAvailability, ActionExecutionContext, ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import type { TabStripTabCreationIntent } from '@orchbee/tab-strip';

import type { WorkbenchTerminalCreateInput } from './workbench-action-ids';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityTabCreation,
	WorkbenchAuthorityTabReceipt,
} from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import type { WorkbenchTerminalCreateRequest } from '../types/workbench-host-actions';

/** The TUI's bound on one Duplicate, however many Tabs it copies. */
const SHARED_TAB_DUPLICATION_MILLISECONDS = 120_000;

interface SharedTabCreationOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

/** Why a Duplicate left some of its Tabs uncopied. */
export type WorkbenchTabDuplicationStop =
	| 'changed'
	| 'deadline'
	| 'not-created'
	| 'terminal-not-started';

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

/** Without a Workspace, only a plain New Tab means anything: it asks for the Workspace first. */
export function getSharedTabCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	intent: TabStripTabCreationIntent,
): ActionAvailability {
	if (runtime.getSnapshot().activeLocation.kind !== 'workspace') {
		return intent.kind === 'new-tab' && intent.groupId === null ? { kind: 'enabled' } : unavailable;
	}
	if (intent.kind !== 'new-tab' || intent.index === undefined) return { kind: 'enabled' };
	// A new Tab is never pinned, so the authority refuses one placed among the pinned Tabs.
	const { tabs } = runtime.getActiveTerminalContext().tabModel.getSnapshot();
	return intent.index < tabs.filter((tab) => tab.pinned).length ? unavailable : { kind: 'enabled' };
}

/** A launch profile only chooses how a Tab starts, so it never stands in for the missing Workspace. */
export function getSharedTerminalCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchTerminalCreateInput,
): ActionAvailability {
	if (input.profile !== undefined && runtime.getSnapshot().activeLocation.kind !== 'workspace') {
		return unavailable;
	}
	return getSharedTabCreationAvailability(runtime, {
		groupId: input.groupId,
		kind: 'new-tab',
		...(input.index === undefined ? {} : { index: input.index }),
	});
}

function withoutReceipt(result: ActionResult<WorkbenchAuthorityTabReceipt>): ActionResult<void> {
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
	const handover = beginTerminalHandover();
	const created = await authority.createTab(context, {
		...creation,
		terminalContextId: runtime.getActiveTerminalContext().terminalContextId,
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
	const { tabs } = options.runtime.getActiveTerminalContext().tabModel.getSnapshot();
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

/** What a Duplicate started from, since it copies what the person was looking at when they asked. */
function attentionOf(runtime: WorkbenchProjectWorkspaceRuntime): string {
	const { activeTabId, selectedTabIds } = runtime.getActiveTerminalContext().tabModel.getSnapshot();
	return JSON.stringify([runtime.getSnapshot().activeLocation, activeTabId, [...selectedTabIds]]);
}

/**
 * One creation per Tab, in the order the strip shows them, each waiting for the one before it. The
 * person stays where they are. Anything uncertain stops the rest, because it is never repeated.
 */
export async function duplicateSharedTabs(
	context: ActionExecutionContext,
	tabIds: readonly string[],
	{ authority, onStopped, runtime }: SharedTabDuplicationOptions,
): Promise<ActionResult<void>> {
	const { tabModel, terminalContextId } = runtime.getActiveTerminalContext();
	const requested = new Set(tabIds);
	const sources = tabModel
		.getSnapshot()
		.tabs.filter((tab) => requested.has(tab.id))
		.map((tab) => tab.id);
	const attention = attentionOf(runtime);
	const deadline = performance.now() + SHARED_TAB_DUPLICATION_MILLISECONDS;
	let created = 0;
	const stopped = (stop: WorkbenchTabDuplicationStop): ActionResult<void> => {
		onStopped(stop, { created, total: sources.length });
		return { failure: { code: `duplication-${stop}` }, kind: 'failed' };
	};
	for (const tabId of sources) {
		if (context.signal.aborted) break;
		if (attentionOf(runtime) !== attention) return stopped('changed');
		if (performance.now() >= deadline) return stopped('deadline');
		const copy = await authority.createTab(context, {
			placement: { kind: 'duplicate', tabId },
			terminalContextId,
		});
		if (copy.kind === 'cancelled') break;
		if (copy.kind !== 'completed') {
			return copy.kind === 'failed' && copy.failure.code === 'outcome-unknown'
				? stopped('terminal-not-started')
				: stopped('not-created');
		}
		created += 1;
		if (copy.value?.terminalStarted !== true) return stopped('terminal-not-started');
	}
	return created === 0 && context.signal.aborted ? { kind: 'cancelled' } : { kind: 'completed' };
}
