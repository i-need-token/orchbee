import type { FileResult } from './file-error';

/** The content a read saw, by VS Code's measure: a save names it as its precondition. */
interface FileVersion {
	readonly mtimeMs: number;
	readonly size: number;
}

/**
 * The text's line ending by VS Code's buffer rule: CRLF when carriage returns exceed half of all
 * line breaks, none without a break.
 */
type FileLineEnding = 'crlf' | 'lf' | 'none';

/** How a read decodes what it reads. */
export interface FileReadOptions {
	/** The encoding for content without a BOM; `utf8` when absent. */
	readonly encoding?: string | undefined;
	/** Decode content that looks binary as text anyway, as VS Code's Open Anyway does. */
	readonly forceText?: boolean | undefined;
	/**
	 * The Client's ceiling in bytes, after VS Code's confirmations: a larger file is `too-large`
	 * before any content arrives.
	 */
	readonly limit: number;
	/** Text decodes for an editor; bytes serve media previews. */
	readonly mode: 'bytes' | 'text';
}

/** What a read returns. `changed` says the file's mtime or size moved while it was read. */
export type FileContent =
	/** A text read of content that looks binary, without `forceText`: no content is sent. */
	| { readonly kind: 'binary'; readonly version: FileVersion }
	| {
			readonly bytes: Uint8Array;
			readonly changed: boolean;
			readonly kind: 'bytes';
			readonly version: FileVersion;
	  }
	| {
			readonly bom: boolean;
			readonly changed: boolean;
			readonly encoding: string;
			readonly kind: 'text';
			readonly lineEnding: FileLineEnding;
			/** The decoder's output; malformed input became U+FFFD. */
			readonly text: string;
			readonly version: FileVersion;
	  };

/** Reading a Workspace's files; writing arrives with editing. */
export interface FileContentPort {
	read(
		request: FileReadOptions & { readonly path: string },
		signal: AbortSignal,
	): Promise<FileResult<FileContent>>;
}
