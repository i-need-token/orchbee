import type { FileBarModule } from './file-bar-contract';

/**
 * The File Bar's only entry. Its views load as their own chunk the first time a window shows the
 * File Bar, so a window that never shows it, and every release build, never evaluates them.
 */
export function loadFileBar(): Promise<FileBarModule> {
	return import('./file-bar-module').then((loaded) => loaded.fileBarModule);
}
