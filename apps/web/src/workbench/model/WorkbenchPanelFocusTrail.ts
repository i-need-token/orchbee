import { findWorkbenchWorkspace } from '../../project-workspace';
import { findPanel } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from './WorkbenchProjectWorkspaceRuntime';

/** Enough to step back through recent work; the oldest focus leaves first, as in the TUI. */
const CAPACITY = 16;

interface WorkbenchPanelFocus {
	readonly panelId: string;
	readonly tabId: string;
	readonly workspaceId: string;
}

/** A Panel Last Panel returns to, with the context that still holds it. */
export interface WorkbenchLastPanelTarget extends WorkbenchPanelFocus {
	readonly context: WorkbenchTerminalContext;
}

/**
 * This window's Panel focus history across the whole Session Workbench, which Last Panel steps back
 * through as the TUI's does. Every settled focus counts, whether a pointer, a key or an Action moved
 * it; a dialog or menu never moves the focused Panel, so it never enters. Most recent first, each
 * Panel at most once; a Panel that is gone leaves the history.
 */
export class WorkbenchPanelFocusTrail {
	private entries: readonly WorkbenchPanelFocus[] = [];
	private readonly runtime: WorkbenchProjectWorkspaceRuntime;

	public constructor(runtime: WorkbenchProjectWorkspaceRuntime) {
		this.runtime = runtime;
	}

	/** Follows the runtime's navigation until the returned function stops it. */
	public readonly observe = (): (() => void) => {
		this.record();
		return this.runtime.subscribeNavigation(this.record);
	};

	/** The most recent other Panel that still exists, or null when there is none. */
	public target(): WorkbenchLastPanelTarget | null {
		const current = this.currentFocus();
		for (const entry of this.entries) {
			if (entry.panelId === current?.panelId) continue;
			const target = this.resolve(entry);
			if (target !== null) return target;
		}
		return null;
	}

	private readonly record = (): void => {
		const current = this.currentFocus();
		const retained = this.entries.filter(
			(entry) => entry.panelId !== current?.panelId && this.resolve(entry) !== null,
		);
		this.entries = (current === null ? retained : [current, ...retained]).slice(0, CAPACITY);
	};

	private currentFocus(): WorkbenchPanelFocus | null {
		const location = this.runtime.getSnapshot().activeLocation;
		if (location.kind !== 'workspace') return null;
		const context = this.runtime.getActiveTerminalContext();
		if (context === null) return null;
		const tabId = context.tabModel.getSnapshot().activeTabId;
		const tab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) return null;
		return { panelId: tab.focusedPanelId, tabId: tab.tabId, workspaceId: location.workspaceId };
	}

	/** The entry's Panel where this window still shows it, in a Workspace it can still open. */
	private resolve(entry: WorkbenchPanelFocus): WorkbenchLastPanelTarget | null {
		const workspace = findWorkbenchWorkspace(this.runtime.getSnapshot(), entry.workspaceId);
		if (workspace === undefined) return null;
		const context = this.runtime.getTerminalContext(workspace.terminalContextId);
		if (context === null) return null;
		if (!context.tabModel.getSnapshot().tabs.some((tab) => tab.id === entry.tabId)) return null;
		const tab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === entry.tabId);
		if (tab === undefined || findPanel(tab.root, entry.panelId) === null) return null;
		return { ...entry, context };
	}
}
