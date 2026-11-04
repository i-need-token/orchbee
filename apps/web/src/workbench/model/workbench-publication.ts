import type { TabModelSnapshot } from '@orchbee/tab-strip/model';

import type { WorkbenchProject, WorkbenchWorkspace } from '../../project-workspace';
import type { PanelLayoutNode } from '../components/workbench-panel-workspace/model/panel-workspace-state';

/** One Tab as published, with the Panels it holds and the names it shows. */
export interface WorkbenchPublishedTab {
	/** The name a person chose for the Tab, which outranks every other. */
	readonly customTitle: string | null;
	readonly group: string | null;
	readonly id: string;
	readonly pinned: boolean;
	/**
	 * Holds no choice of this Client and nothing a terminal reported: every Panel shows its primary
	 * terminal, and every terminal's observed facts are still unknown.
	 */
	readonly root: PanelLayoutNode;
	/** The name the Tab shows while nobody has chosen one. */
	readonly title: string;
}

export interface WorkbenchPublishedTerminalContext {
	/** Changes exactly when the context does, so an unchanged context keeps its live models. */
	readonly fingerprint: string;
	readonly groups: readonly Omit<TabModelSnapshot['groups'][number], 'attention' | 'collapsed'>[];
	readonly tabs: readonly WorkbenchPublishedTab[];
	readonly terminalContextId: string;
	/**
	 * Tabs the authority still holds that this window leaves out for now, such as one whose close it
	 * follows. This Client keeps its view of each, which the Tab takes up again if it returns.
	 */
	readonly withheldTabIds: readonly string[];
}

/**
 * One state published by an authority outside this runtime, in the shapes the runtime's stores hold.
 * The runtime takes all of it as shared truth and keeps only what its own Client owns: where it is,
 * what it expands, selects and focuses, and what a mounted terminal reports about itself.
 */
export interface WorkbenchPublication {
	/** In display order. */
	readonly projects: readonly WorkbenchProject[];
	/** In canonical identity order, one for each Workspace. */
	readonly terminalContexts: readonly WorkbenchPublishedTerminalContext[];
	/** In display order within each Project and within Recents. */
	readonly workspaces: readonly WorkbenchWorkspace[];
}
