import type { FilePathRules } from '../file-resource';
import { filePathKey } from '../filePathKey';
import type { FileChange } from '../file-session-port';
import type { FileEntry } from '../file-tree-port';
import { memoryContentSize, type MemoryContent } from './memory-content';
import { filePathName, filePathParent, isFilePathWithin } from '../file-paths';

/** How a test writes a file. */
export interface MemoryFileOptions {
	/** Text that starts with a BOM in `encoding`. */
	readonly bom?: boolean | undefined;
	/** The encoding a read reports for the text; `utf8` without one. */
	readonly encoding?: string | undefined;
	readonly readonly?: boolean | undefined;
	readonly symlink?: boolean | undefined;
}

interface MemoryDirectory {
	readonly kind: 'directory';
	readonly mtimeMs: number;
	readonly path: string;
	readonly symlink: boolean;
}

interface MemoryFile extends MemoryContent {
	readonly kind: 'file';
	readonly path: string;
	readonly readonly: boolean;
	readonly symlink: boolean;
}

export type MemoryNode = MemoryDirectory | MemoryFile;

const START_TIME_MS = 1_700_000_000_000;
const TICK_MS = 1000;

/**
 * One machine's files for the in-memory session. Every change ticks a deterministic clock, so
 * each write moves its file's mtime.
 */
export class MemoryFileSystem {
	public readonly rules: FilePathRules;
	private readonly nodes = new Map<string, MemoryNode>();
	private time = START_TIME_MS;

	public constructor(rules: FilePathRules) {
		this.rules = rules;
	}

	public node(path: string): MemoryNode | null {
		return this.nodes.get(filePathKey(path, this.rules)) ?? null;
	}

	public children(path: string): readonly MemoryNode[] {
		const key = filePathKey(path, this.rules);
		const children: MemoryNode[] = [];
		for (const node of this.nodes.values()) {
			const parent = filePathParent(node.path, this.rules);
			if (parent !== null && filePathKey(parent, this.rules) === key) children.push(node);
		}
		return children;
	}

	public entry(node: MemoryNode): FileEntry {
		return {
			mtimeMs: node.mtimeMs,
			name: filePathName(node.path, this.rules),
			readonly: node.kind === 'file' && node.readonly,
			size: node.kind === 'file' ? memoryContentSize(node) : 0,
			symlink: node.symlink,
			type: node.kind,
			unrepresentable: false,
		};
	}

	/** Creates the folder and its missing ancestors, and returns what changed. */
	public makeDirectory(path: string): FileChange[] {
		const existing = this.node(path);
		if (existing?.kind === 'file') throw new Error('A file stands where the folder would go.');
		if (existing !== null) return [];
		const parent = filePathParent(path, this.rules);
		const changes = parent === null ? [] : this.makeDirectory(parent);
		this.nodes.set(filePathKey(path, this.rules), {
			kind: 'directory',
			mtimeMs: this.tick(),
			path,
			symlink: false,
		});
		changes.push({ directory: true, path, type: 'added' });
		return changes;
	}

	/** Writes a file, creating its missing ancestors, and returns what changed. */
	public write(
		path: string,
		content: string | Uint8Array,
		options: MemoryFileOptions = {},
	): FileChange[] {
		const existing = this.node(path);
		if (existing?.kind === 'directory') throw new Error('A folder stands where the file would go.');
		const parent = filePathParent(path, this.rules);
		const changes = parent === null ? [] : this.makeDirectory(parent);
		// An overwrite keeps the name's existing spelling, as a case-insensitive file system does.
		const spelling = existing?.path ?? path;
		this.nodes.set(filePathKey(path, this.rules), {
			bom: options.bom ?? false,
			content,
			encoding: options.encoding ?? null,
			kind: 'file',
			mtimeMs: this.tick(),
			path: spelling,
			readonly: options.readonly ?? false,
			symlink: options.symlink ?? false,
		});
		changes.push({
			directory: false,
			path: spelling,
			type: existing === null ? 'added' : 'changed',
		});
		return changes;
	}

	/**
	 * Removes a file, or a folder with everything below it, and returns a deletion for each
	 * removed path, the named one first.
	 */
	public remove(path: string): FileChange[] {
		const existing = this.node(path);
		if (existing === null) return [];
		const changes: FileChange[] = [];
		for (const [key, node] of this.nodes) {
			if (!isFilePathWithin(node.path, existing.path, this.rules)) continue;
			this.nodes.delete(key);
			const change: FileChange = {
				directory: node.kind === 'directory',
				path: node.path,
				type: 'deleted',
			};
			if (node === existing) changes.unshift(change);
			else changes.push(change);
		}
		return changes;
	}

	private tick(): number {
		this.time += TICK_MS;
		return this.time;
	}
}
