/** The operating system of the machine that holds a Workspace's files, as its root facts name it. */
type FileOs = 'linux' | 'macos' | 'windows';

/**
 * How that machine spells and compares paths. Paths are absolute in its native form and keep the
 * backend's spelling; the Client never normalizes them.
 */
export interface FilePathRules {
	/** Linux compares paths bytewise; Windows and macOS ignore case, as VS Code does. */
	readonly caseSensitive: boolean;
	readonly os: FileOs;
	readonly separator: '/' | '\\';
}

/**
 * A file on the Workspace's machine: its path as the backend spells it and the key it compares by,
 * `filePathKey` of that path under the machine's rules. Two resources are one file exactly when
 * their keys are equal.
 */
export interface FileResource {
	readonly key: string;
	readonly path: string;
}
