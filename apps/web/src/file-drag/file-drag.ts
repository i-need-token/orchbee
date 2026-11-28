import type { FilePathRules } from '../file-session';
import { PointerDragKind } from '../pointer-drag';

/** What a drag of files and folders from the File Bar's views carries. */
export interface FileDrag {
	/** The files and folders, as absolute paths in their machine's spelling, in their view's order. */
	readonly paths: readonly string[];
	/** How the machine that holds them spells and compares paths. */
	readonly rules: FilePathRules;
}

/** Drags of the File Bar's files and folders, which a terminal takes as their paths. */
export const FILE_DRAG = new PointerDragKind<FileDrag>();
