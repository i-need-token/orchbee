export { FILE_OVERLAY_ACTION_IDS, fileOverlayOpenInput } from './file-overlay-actions';
export type { FileOverlayActionCatalog } from './file-overlay-actions';
export { readFileOverlayPipPosition, splitFileTabPath, writeFileOverlayPipPosition } from './model';
export type {
	FileOverlayEvent,
	FileOverlayPipPosition,
	FileOverlayPipStorage,
	FileOverlayState,
} from './model';
export { loadFileOverlay } from './view';
export type {
	FileOverlayCommandSource,
	FileOverlayCommands,
	FileOverlayFocus,
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
	FileOverlayModule,
	FileOverlayRegionElements,
	FileOverlayShortcutLabels,
	FileOverlayTitleActions,
	FileOverlayTitleButton,
} from './view';
export { FileOverlayRegistry } from './workspace-stores';
export type { FileOverlayStore } from './workspace-stores';
