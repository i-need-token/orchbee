export { createFileOverlayState, transitionFileOverlay } from './file-overlay-state';
export type { FileOverlayEffect, FileOverlayEvent, FileOverlayState } from './file-overlay-state';
export { fileOverlayCover } from './fileOverlayCover';
export type { FileOverlayBox, FileOverlayRegions } from './fileOverlayCover';
export { fileOverlayEdgeTier } from './fileOverlayEdgeTier';
export { fileOverlayPipBox, fileOverlayPipPosition, moveFileOverlayPip } from './file-overlay-pip';
export type { FileOverlayPipPosition } from './file-overlay-pip';
export {
	readFileOverlayPipPosition,
	writeFileOverlayPipPosition,
} from './file-overlay-pip-storage';
export type { FileOverlayPipStorage } from './file-overlay-pip-storage';
export type { FileOverlayNarrowTier } from './overlay-mode';
export { fileTabLabels, splitFileTabPath } from './file-tab-labels';
