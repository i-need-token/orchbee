import type { ActionAvailability, ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { DroppedPathPlatform } from '@orchbee/terminal/dropped-paths';

/** Paths typed into a terminal, for the shell of the machine that holds them. */
export interface ExplorerTerminalPaths {
	readonly paths: readonly string[];
	readonly platform: DroppedPathPlatform;
}

/** Where Open in Integrated Terminal opens a Tab: a directory, in the Workspace whose tree holds it. */
interface ExplorerTerminalTab {
	readonly cwd: string;
	readonly workspaceId: string;
}

/**
 * The Workbench's terminals, where the Explorer's terminal commands act: VS Code's Open in
 * Integrated Terminal opens Tabs, and Orchbee's Insert Path into Terminal types into the current
 * terminal.
 */
export interface ExplorerTerminals {
	/** Types the paths into the current terminal as a drop on it does, which gives it the keys. */
	readonly insertPaths: (
		context: ActionExecutionContext,
		paths: ExplorerTerminalPaths,
	) => Promise<ActionResult<void>>;
	/** Whether the current terminal takes the paths now; hidden while there is none. */
	readonly insertPathsAvailability: (paths: ExplorerTerminalPaths) => ActionAvailability;
	/** Opens a Tab at the directory as New Tab opens one, which selects it and gives it the keys. */
	readonly openTab: (
		context: ActionExecutionContext,
		tab: ExplorerTerminalTab,
	) => Promise<ActionResult<void>>;
}
