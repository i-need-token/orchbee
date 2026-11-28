import type { FileError } from '../../file-session';

/** A folder the Explorer could not list, which collapses its row; VS Code notifies the person. */
export class ExplorerListError extends Error {
	readonly error: FileError;
	readonly path: string;

	constructor(path: string, error: FileError) {
		super(`The Explorer could not list a folder: ${error.code}.`);
		this.name = 'ExplorerListError';
		this.error = error;
		this.path = path;
	}
}
