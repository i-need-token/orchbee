import type { FileOverlayModule } from './file-overlay-contract';

/**
 * The only entry to the overlay's components. They load as their own chunk the first time a
 * Workspace's overlay or the File Bar shows, so a window that shows neither, and every release
 * build, never evaluates them.
 */
export function loadFileOverlay(): Promise<FileOverlayModule> {
	return import('./file-overlay-module').then((loaded) => loaded.fileOverlayModule);
}
