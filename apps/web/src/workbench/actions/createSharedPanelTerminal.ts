import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';

import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityPanelTerminalCreation,
} from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';

interface SharedPanelTerminalOptions {
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

/** What asking for a new Panel terminal settles with: the Panel that shows it, when it is known. */
export type WorkbenchSharedPanelTerminalCreate = (
	context: ActionExecutionContext,
	creation: WorkbenchAuthorityPanelTerminalCreation,
) => Promise<ActionResult<string>>;

/**
 * Shows the new terminal and gives it the keyboard, unless the person has moved on while the authority
 * decided: late completion never takes focus or navigation from later work. The authority states
 * nothing about this Client's selection, so choosing what to show stays here.
 */
export async function createSharedPanelTerminal(
	context: ActionExecutionContext,
	creation: WorkbenchAuthorityPanelTerminalCreation,
	{ authority, beginTerminalHandover, runtime }: SharedPanelTerminalOptions,
): Promise<ActionResult<string>> {
	const origin = runtime.getSnapshot().activeLocation;
	const handover = beginTerminalHandover();
	const created = await authority.createPanelTerminal(context, creation);
	if (created.kind === 'unchanged') return { kind: 'unchanged' };
	if (created.kind !== 'completed') return created;
	if (created.value === undefined) return { kind: 'completed' };
	const { panelId, tabId, terminalContextId } = created.value;
	// A publication replaces the store this Client held when it asked.
	const store = runtime.getTerminalContext(terminalContextId)?.panelWorkspaceStore;
	if (
		store !== undefined &&
		store.getActiveTabId() === tabId &&
		!context.signal.aborted &&
		handover.held() &&
		workbenchActiveLocationsEqual(origin, runtime.getSnapshot().activeLocation)
	) {
		store.transition({ panelId, tabId, type: 'panel-revealed' });
		handover.complete();
	}
	return { kind: 'completed', value: panelId };
}
