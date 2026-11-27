import * as React from 'react';

import { FileOverlayRegistry } from '../../file-overlay';
import type { WorkbenchWorkspace } from '../../project-workspace';
import type { WorkbenchTerminalContext } from '../model/WorkbenchProjectWorkspaceRuntime';

interface UseWorkbenchFileOverlayOptions {
	/** Whether this window offers the File Bar; without the offer no overlay exists or listens. */
	readonly offered: boolean;
	/** Whether the Sidebar shows docked, which offers left-expand while the File Bar is on the right. */
	readonly sidebarDocked: boolean;
	/** The active Workspace's terminals, which the terminal area shows; null outside a Workspace. */
	readonly terminalContext: WorkbenchTerminalContext | null;
	/** The active Workspace; null outside a Workspace. */
	readonly workspaceId: string | null;
	/** The window's Workspaces, whose overlays live while they exist. */
	readonly workspaces: readonly WorkbenchWorkspace[];
}

const subscribeToNothing = (): (() => void) => () => undefined;
const noActiveTab = (): null => null;

/**
 * Keeps one file overlay per Workspace for this Workbench and tells the overlays what the Workbench
 * changes beneath them: the terminal the active Workspace shows, whether left-expand is offered, and
 * which Workspaces still exist.
 */
export function useWorkbenchFileOverlay({
	offered,
	sidebarDocked,
	terminalContext,
	workspaceId,
	workspaces,
}: UseWorkbenchFileOverlayOptions): void {
	const [registry] = React.useState(() =>
		offered ? new FileOverlayRegistry(sidebarDocked) : null,
	);
	const panels = registry === null ? null : (terminalContext?.panelWorkspaceStore ?? null);
	const activeTabId = React.useSyncExternalStore(
		panels?.subscribe ?? subscribeToNothing,
		panels?.getActiveTabId ?? noActiveTab,
		panels?.getActiveTabId ?? noActiveTab,
	);
	const terminalContextId = terminalContext?.terminalContextId ?? null;
	React.useEffect(() => {
		registry?.showTerminal(
			workspaceId === null ? null : { tabId: activeTabId, terminalContextId, workspaceId },
		);
	}, [activeTabId, registry, terminalContextId, workspaceId]);
	React.useEffect(() => {
		registry?.setLeftExpandOffered(sidebarDocked);
	}, [registry, sidebarDocked]);
	React.useEffect(() => {
		registry?.retain(new Set(workspaces.map((workspace) => workspace.id)));
	}, [registry, workspaces]);
}
