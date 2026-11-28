export { FILE_OVERLAY_ACTION_IDS, fileOverlayOpenInput } from './file-overlay-actions';
export type { FileOverlayActionCatalog, FileOverlayOpenFile } from './file-overlay-actions';
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
	FileOverlayEditorHost,
	FileOverlayFocus,
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
	FileOverlayModule,
	FileOverlayRegionElements,
	FileOverlayShortcutLabels,
	FileOverlayTitleActions,
	FileOverlayTitleButton,
} from './view';
export {
	dispatchFileOverlayEvent,
	FileOverlayRegistry,
	FileTabContentRegistry,
} from './workspace-stores';
export type { FileOverlayStore, FileTabContentPort, FileTabContents } from './workspace-stores';
