import type { FileError, FileResult } from '../file-error';
import type { FileGrantSource } from '../file-grant-port';
import type { FilePathRules } from '../file-resource';
import { filePathKey } from '../filePathKey';
import type {
	FileCapability,
	FileChange,
	FileRoot,
	FileRootEvent,
	FileSessionPort,
	FileWorkspaceSession,
} from '../file-session-port';
import type { FileListPage, FileStatItem } from '../file-tree-port';
import { MemoryAnswering } from './MemoryAnswering';
import { MemoryFileSystem, type MemoryFileOptions, type MemoryNode } from './MemoryFileSystem';
import { MemoryGit } from './MemoryGit';
import { memoryVersion, readMemoryContent } from './memory-content';
import { filePathChild, filePathParent, isFilePathWithin } from '../file-paths';
import type { MemoryRootStream, MemorySessionState } from './memory-session-state';

type FileRootUpdate = Extract<FileRootEvent, { readonly kind: 'update' }>;
type FileWatchState = NonNullable<FileRootUpdate['watch']>;
type FileListing = FileListPage['descendants'][number];

/** How the in-memory machine behaves. */
interface MemoryFileSessionOptions {
	/** `manual` holds every answer until `flush`; `immediate`, the default, answers in order. */
	readonly answering?: 'immediate' | 'manual' | undefined;
	/** The negotiated capabilities; `files` alone by default. */
	readonly capabilities?: readonly FileCapability[] | undefined;
	readonly machine?: string | undefined;
	/** Entries per listing and status page; the backend's bound, 2,000, by default. */
	readonly pageSize?: number | undefined;
	/** The machine's path rules; Linux's by default. */
	readonly rules?: FilePathRules | undefined;
}

/** A grant the machine admitted, in the order it admitted them. */
interface MemoryGrant {
	readonly path: string;
	readonly source: FileGrantSource;
	readonly workspaceId: string;
}

interface MemoryListCursor {
	readonly offset: number;
	readonly path: string;
	readonly session: MemorySessionState;
}

const LINUX_RULES: FilePathRules = { caseSensitive: true, os: 'linux', separator: '/' };
const PAGE_SIZE = 2000;
const PATHS_PER_REQUEST = 256;
const SINGLE_CHILD_DEPTH = 64;

function failure<Value>(code: FileError['code']): FileResult<Value> {
	return { error: { code }, kind: 'error' };
}

function ok<Value>(value: Value): FileResult<Value> {
	return { kind: 'ok', value };
}

function sameRoot(left: FileRoot, right: FileRoot): boolean {
	if (left.state === 'none' || right.state === 'none') return left.state === right.state;
	return left.state === right.state && left.path === right.path;
}

/**
 * A deterministic File Bar backend for tests: one machine's files and Git, served to any number
 * of Workspace sessions under the backend's rules. Requests answer in order, or wait for `flush`;
 * listing loads a folder, and only loaded folders and opened files reach the root stream; a path
 * outside the root, the consented repositories and the opened files is `outside-scope`; and a
 * dropped link releases what the connection held.
 *
 * @internal Tests drive it; production sessions come from the wire adapters.
 */
export class MemoryFileSessionPort implements FileSessionPort {
	public readonly git: MemoryGit;
	private readonly admitted: MemoryGrant[] = [];
	private readonly answering: MemoryAnswering;
	private readonly capabilities: ReadonlySet<FileCapability>;
	/** Consented repositories by comparison key; consent is kept per machine. */
	private readonly consented = new Map<string, string>();
	private readonly files: MemoryFileSystem;
	private readonly listCursors = new Map<string, MemoryListCursor>();
	private readonly machine: string;
	private nextCursor = 1;
	private readonly pageSize: number;
	private readonly roots = new Map<string, string>();
	private readonly sessions: MemorySessionState[] = [];
	private readonly watches = new Map<string, FileWatchState>();

	public constructor(options: MemoryFileSessionOptions = {}) {
		this.answering = new MemoryAnswering(options.answering ?? 'immediate');
		this.capabilities = new Set(options.capabilities ?? ['files']);
		this.files = new MemoryFileSystem(options.rules ?? LINUX_RULES);
		this.machine = options.machine ?? 'local';
		this.pageSize = options.pageSize ?? PAGE_SIZE;
		this.git = new MemoryGit({
			answering: this.answering,
			pageSize: this.pageSize,
			sessionsOf: (workspaceId) => this.openSessions(workspaceId),
		});
	}

	/** Requests waiting for their answers. */
	public get waiting(): number {
		return this.answering.size;
	}

	/** Gives every waiting answer in request order and returns how many it gave. */
	public flush(): number {
		return this.answering.flush();
	}

	/** The Workspaces with an open session, once for each session, in opening order. */
	public openWorkspaces(): readonly string[] {
		return this.sessions.map((session) => session.workspaceId);
	}

	/** The grants admitted so far, in order. */
	public grants(): readonly MemoryGrant[] {
		return [...this.admitted];
	}

	/** Sets a Workspace's root directory, or none; its streams report the move. */
	public setRoot(workspaceId: string, path: string | null): void {
		if (path === null) this.roots.delete(workspaceId);
		else this.roots.set(workspaceId, path);
		this.publish([], workspaceId);
	}

	/** Reports how fully a Workspace's root is watched. */
	public setWatch(workspaceId: string, watch: FileWatchState): void {
		this.watches.set(workspaceId, watch);
		for (const stream of this.readyStreams(workspaceId)) {
			this.sendUpdate(stream, { changes: [], rescan: [], root: null, watch });
		}
	}

	/** Reports that a Workspace dropped changes in these folders, as a watcher overflow does. */
	public rescan(workspaceId: string, folders: readonly string[]): void {
		for (const session of this.openSessions(workspaceId)) {
			const rescan = folders.filter((folder) =>
				session.loaded.has(filePathKey(folder, this.files.rules)),
			);
			if (rescan.length === 0) continue;
			for (const stream of session.rootStreams) {
				if (stream.ready) this.sendUpdate(stream, { changes: [], rescan, root: null, watch: null });
			}
		}
	}

	public writeFile(path: string, content: string | Uint8Array, options?: MemoryFileOptions): void {
		this.publish(this.files.write(path, content, options), null);
	}

	public makeDirectory(path: string): void {
		this.publish(this.files.makeDirectory(path), null);
	}

	public remove(path: string): void {
		this.publish(this.files.remove(path), null);
	}

	/**
	 * Drops the link. Waiting requests and every request until `reconnect` settle
	 * `remote-disconnected`; loaded folders, opened files and cursors are released; streams get a
	 * gap. With `delivered`, the waiting requests ran and only their replies were lost.
	 */
	public disconnect(options: { readonly delivered?: boolean | undefined } = {}): void {
		this.answering.disconnect(options.delivered === true);
		this.listCursors.clear();
		for (const session of this.sessions) {
			session.loaded.clear();
			session.granted.clear();
			for (const stream of session.rootStreams) {
				if (stream.ready) stream.listener({ kind: 'gap' });
				stream.ready = false;
			}
		}
		this.git.disconnect(this.sessions);
	}

	/** Restores the link: every stream starts again with a snapshot or summary. */
	public reconnect(): void {
		this.answering.reconnect();
		for (const session of this.sessions) {
			for (const stream of session.rootStreams) this.startRoot(stream, session.workspaceId);
		}
		this.git.reconnect(this.sessions);
	}

	public open(workspaceId: string): FileWorkspaceSession {
		const session: MemorySessionState = {
			closed: false,
			granted: new Set(),
			loaded: new Set(),
			relayStreams: new Set(),
			rootStreams: new Set(),
			summaryStreams: new Set(),
			workspaceId,
		};
		this.sessions.push(session);
		const rules = this.files.rules;
		return {
			capabilities: this.capabilities,
			close: () => this.close(session),
			content: {
				read: (request, signal) => {
					const issued = this.files.node(request.path);
					return this.answering.answer(session, signal, () => {
						if (!this.readable(session, request.path)) return failure('outside-scope');
						const node = this.files.node(request.path);
						if (node === null) return failure('not-found');
						if (node.kind === 'directory') return failure('is-directory');
						const version = issued?.kind === 'file' ? issued : node;
						return readMemoryContent(node, request, memoryVersion(version));
					});
				},
			},
			git: this.git.createPort(session),
			grants: {
				closeRepository: (repository, signal) =>
					this.answering.answer(session, signal, () => {
						this.consented.delete(filePathKey(repository, rules));
						return ok(null);
					}),
				consentRepository: (repository, signal) =>
					this.answering.answer(session, signal, () => {
						if (this.files.node(repository)?.kind !== 'directory') return failure('not-found');
						this.consented.set(filePathKey(repository, rules), repository);
						return ok(null);
					}),
				grant: (path, source, signal) =>
					this.answering.answer(session, signal, () => {
						const node = this.files.node(path);
						if (node === null) return failure('not-found');
						if (node.kind === 'directory') return failure('is-directory');
						session.granted.add(filePathKey(path, rules));
						this.admitted.push({ path, source, workspaceId });
						return ok(this.files.entry(node));
					}),
				ungrant: (paths) => {
					for (const path of paths) session.granted.delete(filePathKey(path, rules));
				},
			},
			machine: this.machine,
			subscribe: (listener) => this.subscribe(session, listener),
			tree: {
				list: (request, signal) =>
					this.answering.answer(session, signal, () => this.list(session, request)),
				release: (paths) => {
					for (const path of paths) session.loaded.delete(filePathKey(path, rules));
				},
				stat: (paths, signal) =>
					this.answering.answer(session, signal, () =>
						paths.length > PATHS_PER_REQUEST
							? failure('invalid-request')
							: ok(paths.map((path) => this.stat(session, path))),
					),
			},
			workspaceId,
		};
	}

	private close(session: MemorySessionState): void {
		if (session.closed) return;
		session.closed = true;
		this.answering.cancel(session);
		this.sessions.splice(this.sessions.indexOf(session), 1);
		for (const [cursor, position] of this.listCursors) {
			if (position.session === session) this.listCursors.delete(cursor);
		}
		this.git.release(session);
		session.granted.clear();
		session.loaded.clear();
		session.relayStreams.clear();
		session.rootStreams.clear();
		session.summaryStreams.clear();
	}

	private subscribe(
		session: MemorySessionState,
		listener: (event: FileRootEvent) => void,
	): () => void {
		if (session.closed) return () => undefined;
		const stream: MemoryRootStream = {
			listener,
			ready: false,
			root: { state: 'none' },
			sequence: 0,
		};
		session.rootStreams.add(stream);
		queueMicrotask(() => {
			if (!session.closed && session.rootStreams.has(stream) && this.answering.connected) {
				this.startRoot(stream, session.workspaceId);
			}
		});
		return () => {
			session.rootStreams.delete(stream);
		};
	}

	/** Sends a stream's snapshot, then its watch state when the root is not fully watched. */
	private startRoot(stream: MemoryRootStream, workspaceId: string): void {
		if (stream.ready) return;
		stream.ready = true;
		stream.sequence += 1;
		stream.root = this.rootOf(workspaceId);
		stream.listener({
			kind: 'snapshot',
			root: stream.root,
			rules: this.files.rules,
			sequence: stream.sequence,
		});
		const watch = this.watches.get(workspaceId) ?? 'full';
		if (watch !== 'full') this.sendUpdate(stream, { changes: [], rescan: [], root: null, watch });
	}

	private list(
		session: MemorySessionState,
		request: Parameters<FileWorkspaceSession['tree']['list']>[0],
	): FileResult<FileListPage> {
		const rules = this.files.rules;
		let offset = 0;
		if (request.cursor !== undefined) {
			const continued = this.listCursors.get(request.cursor);
			if (
				continued?.session !== session ||
				filePathKey(continued.path, rules) !== filePathKey(request.path, rules)
			) {
				return failure('invalid-request');
			}
			this.listCursors.delete(request.cursor);
			offset = continued.offset;
		}
		if (!this.browsable(session, request.path)) return failure('outside-scope');
		const node = this.files.node(request.path);
		if (node === null) return failure('not-found');
		if (node.kind !== 'directory') return failure('not-directory');
		session.loaded.add(filePathKey(request.path, rules));
		const children = this.files.children(request.path);
		const end = offset + this.pageSize;
		const entries = children.slice(offset, end).map((child) => this.files.entry(child));
		let cursor: string | null = null;
		if (end < children.length) {
			cursor = `list-${this.nextCursor}`;
			this.nextCursor += 1;
			this.listCursors.set(cursor, { offset: end, path: request.path, session });
		}
		const descendants =
			request.singleChild === true && offset === 0
				? this.singleChildChain(session, request.path, children)
				: [];
		return ok({ cursor, descendants, entries });
	}

	/** Lists, and so loads, each folder of the single-child chain below a folder. */
	private singleChildChain(
		session: MemorySessionState,
		path: string,
		children: readonly MemoryNode[],
	): FileListing[] {
		const rules = this.files.rules;
		const chain: FileListing[] = [];
		let folder = path;
		let current = children;
		for (let depth = 0; depth < SINGLE_CHILD_DEPTH; depth += 1) {
			const only = current.length === 1 ? current[0] : undefined;
			if (only?.kind !== 'directory') break;
			folder = filePathChild(folder, this.files.entry(only).name, rules);
			current = this.files.children(folder);
			session.loaded.add(filePathKey(folder, rules));
			chain.push({ entries: current.map((child) => this.files.entry(child)), path: folder });
		}
		return chain;
	}

	private stat(session: MemorySessionState, path: string): FileStatItem {
		if (!this.readable(session, path))
			return { error: { code: 'outside-scope' }, kind: 'error', path };
		const node = this.files.node(path);
		if (node === null) return { error: { code: 'not-found' }, kind: 'error', path };
		return { entry: this.files.entry(node), kind: 'entry', path };
	}

	/** Whether a path lies inside the root or a consented repository. */
	private browsable(session: MemorySessionState, path: string): boolean {
		const rules = this.files.rules;
		const root = this.roots.get(session.workspaceId);
		if (root !== undefined && isFilePathWithin(path, root, rules)) return true;
		for (const repository of this.consented.values()) {
			if (isFilePathWithin(path, repository, rules)) return true;
		}
		return false;
	}

	/** Whether a path can be read: browsable, or an opened file. */
	private readable(session: MemorySessionState, path: string): boolean {
		return (
			session.granted.has(filePathKey(path, this.files.rules)) || this.browsable(session, path)
		);
	}

	private rootOf(workspaceId: string): FileRoot {
		const path = this.roots.get(workspaceId);
		if (path === undefined) return { state: 'none' };
		return { path, state: this.files.node(path)?.kind === 'directory' ? 'present' : 'missing' };
	}

	/**
	 * Sends each session the changes that reach it: those in its loaded folders and to its opened
	 * files, without the contents of a deleted folder, as VS Code coalesces them. A stream whose
	 * root moved also gets the new root; `workspaceId` limits the update to one Workspace.
	 */
	private publish(changes: readonly FileChange[], workspaceId: string | null): void {
		const rules = this.files.rules;
		for (const session of this.sessions) {
			if (workspaceId !== null && session.workspaceId !== workspaceId) continue;
			const reaching = changes.filter((change) => {
				if (session.granted.has(filePathKey(change.path, rules))) return true;
				const parent = filePathParent(change.path, rules);
				return parent !== null && session.loaded.has(filePathKey(parent, rules));
			});
			const deletedFolders = reaching.filter(
				(change) => change.type === 'deleted' && change.directory,
			);
			const coalesced = reaching.filter(
				(change) =>
					!deletedFolders.some(
						(folder) => folder !== change && isFilePathWithin(change.path, folder.path, rules),
					),
			);
			const root = this.rootOf(session.workspaceId);
			for (const stream of session.rootStreams) {
				if (!stream.ready) continue;
				const moved = !sameRoot(root, stream.root);
				if (coalesced.length === 0 && !moved) continue;
				stream.root = root;
				this.sendUpdate(stream, {
					changes: coalesced,
					rescan: [],
					root: moved ? root : null,
					watch: null,
				});
			}
		}
	}

	private sendUpdate(
		stream: MemoryRootStream,
		update: Omit<FileRootUpdate, 'kind' | 'sequence'>,
	): void {
		stream.sequence += 1;
		stream.listener({ ...update, kind: 'update', sequence: stream.sequence });
	}

	private openSessions(workspaceId: string): readonly MemorySessionState[] {
		return this.sessions.filter((session) => session.workspaceId === workspaceId);
	}

	private readyStreams(workspaceId: string): readonly MemoryRootStream[] {
		return this.openSessions(workspaceId).flatMap((session) =>
			[...session.rootStreams].filter((stream) => stream.ready),
		);
	}
}
