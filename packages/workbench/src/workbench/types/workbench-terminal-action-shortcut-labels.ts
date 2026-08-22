import type { WorkbenchTerminalActionId } from '../actions/terminal-action-contracts';

export type WorkbenchTerminalActionShortcutLabels = Readonly<
	Partial<Record<WorkbenchTerminalActionId, string>>
>;
