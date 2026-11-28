export {
	createProjectedTerminalActionComposition,
	type ProjectedTerminalActionResultEvent,
} from './createProjectedTerminalActionComposition';
export { PROJECTED_TERMINAL_ACTION_IDS } from './projected-terminal-action-ids';
export type { ProjectedTerminalControlPresentation } from './registerProjectedTerminalControlAction';
export type { ProjectedTerminalFileOpener } from './registerProjectedTerminalActions';
export { installProjectedTerminalShortcuts } from './installProjectedTerminalShortcuts';
export type {
	ProjectedTerminalShortcutBindingSource,
	ProjectedTerminalShortcutPlatform,
} from '../../projected-terminal-keyboard-shortcuts';
