import { FileBarColumn } from './components/file-bar-column/FileBarColumn';
import { FileBarToggleBadge } from './components/file-bar-toggle-badge/FileBarToggleBadge';
import type { FileBarModule } from './file-bar-contract';

/** The root of the File Bar's chunk: nothing outside this module reaches its views statically. */
export const fileBarModule: FileBarModule = { FileBarColumn, FileBarToggleBadge };
