import type { FileOverlayRegions } from './fileOverlayCover';
import type { FileOverlayNarrowTier } from './overlay-mode';

/**
 * The tier that the frame's left edge, dragged to `x` in the Workbench root, previews: left-expand
 * once the pointer crosses the docked Sidebar's midline, default on the midline's other side.
 */
export function fileOverlayEdgeTier(regions: FileOverlayRegions, x: number): FileOverlayNarrowTier {
	const { sidebar } = regions;
	return sidebar !== null && x < (sidebar.left + sidebar.right) / 2 ? 'left-expand' : 'default';
}
