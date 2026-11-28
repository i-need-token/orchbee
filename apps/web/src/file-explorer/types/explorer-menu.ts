import type { ActionExecution, ActionInvocationSource } from '@orchbee/actions';

import type { ExplorerMenuCommand } from '../model/explorer-menus';

/**
 * Runs an Action a person invoked from the Explorer's menu, and says its refusal or failure when no
 * surface said it while the Action ran.
 */
export type FileExplorerReportActionOutcome = (
	source: ActionInvocationSource,
	start: () => ActionExecution<unknown>,
) => void;

/** One row of the Explorer's context menu, which runs its command's Action. */
export interface ExplorerMenuItem {
	readonly disabled: boolean;
	readonly id: ExplorerMenuCommand;
	readonly label: string;
	readonly run: () => void;
	/** The keys that run the command's Action now, which VS Code's menu shows beside a command. */
	readonly shortcut: string | undefined;
}

/** The rows between two separators. */
export type ExplorerMenuGroup = readonly ExplorerMenuItem[];
