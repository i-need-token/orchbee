import type {
	FileCapability,
	FileContentPort,
	FileEntry,
	FileGrantPort,
	FileGrantSource,
	FileListPage,
	FileResult,
	FileRootEvent,
	FileStatItem,
	FileTreePort,
	FileWorkspaceSession,
} from '../../../file-session';
import { FILES_OPERATIONS } from '../../server-connection';
import type { ServerFileChannel, ServerFileServiceFollower } from './ServerFileChannel';
import { ServerFileRootStream } from './ServerFileRootStream';
import { ServerGitPort } from './ServerGitPort';
import { readServerFile } from './readServerFile';
import { CANCELLED, fileFailure, pathBatches, untilAborted } from './server-file-exchange';

type FileListRequest = Parameters<FileTreePort['list']>[0];

/** One grant the session asked for, which the Server counts while it holds it. */
interface GrantRecord {
	/** The person let it go while it was being asked for; it is released once it lands. */
	released: boolean;
	readonly source: FileGrantSource;
	/** Asked for, held by the Server, or lost when the service restarted. */
	state: 'held' | 'lost' | 'pending';
}

interface ServerFileSessionOptions {
	readonly capabilities: ReadonlySet<FileCapability>;
	readonly channel: ServerFileChannel;
	readonly machine: string;
	/** The session closed, so its port forgets it. */
	readonly onClosed: () => void;
	readonly workspaceId: string;
}

/** The paths `fs.stat` takes in one call, by the backend's bound. */
const PATHS_PER_STAT = 256;

/**
 * One Workspace's file session over a Server connection. A request the caller gives up, or one
 * still running when the session closes, settles `cancelled` at once, but what the Server did for it
 * is still undone: a folder its listing loaded is released and a file it granted is ungranted. The
 * Server counts each grant, so this keeps one record per grant and ungrants each once; when the file
 * service restarts, which drops them, it grants them again once the service answers.
 */
export class ServerFileSession implements FileWorkspaceSession, ServerFileServiceFollower {
	readonly capabilities: ReadonlySet<FileCapability>;
	readonly content: FileContentPort;
	readonly git: ServerGitPort;
	readonly grants: FileGrantPort;
	readonly machine: string;
	readonly tree: FileTreePort;
	readonly workspaceId: string;
	readonly #abort = new AbortController();
	readonly #channel: ServerFileChannel;
	#closed = false;
	/** Every grant asked for and not yet given back, by path as it was granted. */
	readonly #granted = new Map<string, GrantRecord[]>();
	/** Folders this session holds loaded. */
	readonly #loaded = new Set<string>();
	readonly #onClosed: () => void;
	readonly #root: ServerFileRootStream;
	readonly #unfollow: () => void;
	/** Folders listed and not released since, which a listing in flight holds once it lands. */
	readonly #wanted = new Set<string>();

	constructor(options: ServerFileSessionOptions) {
		this.capabilities = options.capabilities;
		this.git = new ServerGitPort(options.channel, options.workspaceId);
		this.machine = options.machine;
		this.workspaceId = options.workspaceId;
		this.#channel = options.channel;
		this.#onClosed = options.onClosed;
		this.#root = new ServerFileRootStream(options.channel, options.workspaceId);
		this.content = {
			read: (request, signal) =>
				this.#run(signal, (combined) =>
					readServerFile(this.#channel, this.workspaceId, request, combined),
				),
		};
		this.grants = {
			closeRepository: (repository, signal) =>
				this.#repository('repository.close', repository, signal),
			consentRepository: (repository, signal) =>
				this.#repository('repository.consent', repository, signal),
			grant: (path, source, signal) => this.#grant(path, source, signal),
			ungrant: (paths) => this.#ungrant(paths),
		};
		this.tree = {
			list: (request, signal) => this.#run(signal, (combined) => this.#list(request, combined)),
			release: (paths) => this.#release(paths),
			stat: (paths, signal) => this.#run(signal, (combined) => this.#stat(paths, combined)),
		};
		this.#unfollow = options.channel.follow(this);
	}

	subscribe(listener: (event: FileRootEvent) => void): () => void {
		return this.#root.subscribe(listener);
	}

	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#abort.abort();
		this.#unfollow();
		this.#root.close();
		this.git.close();
		const { loads } = this.#channel;
		for (const folder of this.#loaded) loads.drop(this.workspaceId, folder);
		this.#loaded.clear();
		this.#wanted.clear();
		const held: string[] = [];
		for (const [path, records] of this.#granted) {
			for (const record of records) {
				if (record.state === 'held') held.push(path);
				// A grant still in flight is given back once it lands.
				else record.released = true;
			}
		}
		this.#granted.clear();
		this.#sendUngrant(held);
		this.#onClosed();
	}

	ended(): void {
		this.#root.end();
	}

	lost(): void {
		for (const records of this.#granted.values()) {
			for (const record of records) {
				if (record.state === 'held') record.state = 'lost';
			}
		}
	}

	served(regained: boolean): void {
		this.#root.served();
		if (!regained || this.#closed) return;
		for (const [path, records] of this.#granted) {
			for (const record of records) {
				if (record.state === 'lost') void this.#ask(path, record);
			}
		}
	}

	/** Runs a request under the caller's signal and the session's, settling `cancelled` on either. */
	#run<Value>(
		signal: AbortSignal,
		request: (combined: AbortSignal) => Promise<FileResult<Value>>,
	): Promise<FileResult<Value>> {
		if (this.#closed) return Promise.resolve(CANCELLED);
		const combined = AbortSignal.any([signal, this.#abort.signal]);
		return untilAborted(request(combined), combined);
	}

	async #list(request: FileListRequest, signal: AbortSignal): Promise<FileResult<FileListPage>> {
		const { path } = request;
		const record = FILES_OPERATIONS['fs.list'].encode({
			cursor: request.cursor,
			op: 'fs.list',
			path,
			singleChild: request.singleChild,
			workspaceId: this.workspaceId,
		});
		if (record === null) return fileFailure('invalid-request');
		this.#wanted.add(path);
		const { loads } = this.#channel;
		await loads.list(this.workspaceId, path);
		const result = await this.#channel.request(
			record,
			(answer) => FILES_OPERATIONS['fs.list'].decode(answer),
			{ signal },
		);
		const loaded =
			result.kind === 'ok'
				? [path, ...result.value.descendants.map((descendant) => descendant.path)]
				: [];
		loads.listed(this.workspaceId, path, loaded);
		// A listing its caller gave up, or one released meanwhile, holds nothing.
		if (!signal.aborted && this.#wanted.has(path)) {
			for (const folder of loaded) {
				if (this.#loaded.has(folder)) continue;
				this.#loaded.add(folder);
				loads.hold(this.workspaceId, folder);
			}
		}
		// A listing nobody wants any longer gives its folders back.
		loads.settle(this.workspaceId, path);
		for (const folder of loaded) loads.settle(this.workspaceId, folder);
		return result;
	}

	#release(paths: readonly string[]): void {
		for (const path of paths) {
			this.#wanted.delete(path);
			if (!this.#loaded.delete(path)) continue;
			this.#channel.loads.drop(this.workspaceId, path);
		}
	}

	async #stat(
		paths: readonly string[],
		signal: AbortSignal,
	): Promise<FileResult<readonly FileStatItem[]>> {
		if (paths.length === 0) return { kind: 'ok', value: [] };
		if (paths.length > PATHS_PER_STAT) return fileFailure('invalid-request');
		// Long paths can outgrow one request's record, so they go in several at once.
		const results = await Promise.all(
			pathBatches(paths).map(async (batch) => {
				const result = await this.#channel.request(
					FILES_OPERATIONS['fs.stat'].encode({
						op: 'fs.stat',
						paths: [...batch],
						workspaceId: this.workspaceId,
					}),
					(answer) => FILES_OPERATIONS['fs.stat'].decode(answer),
					{ signal },
				);
				// Each path has its item, in the order asked.
				if (result.kind === 'ok' && result.value.length !== batch.length) {
					return fileFailure('failed');
				}
				return result;
			}),
		);
		const items: FileStatItem[] = [];
		for (const result of results) {
			if (result.kind === 'error') return result;
			items.push(...result.value);
		}
		return { kind: 'ok', value: items };
	}

	#grant(
		path: string,
		source: FileGrantSource,
		signal: AbortSignal,
	): Promise<FileResult<FileEntry>> {
		if (this.#closed || signal.aborted) return Promise.resolve(CANCELLED);
		const record: GrantRecord = { released: false, source, state: 'pending' };
		let records = this.#granted.get(path);
		if (records === undefined) {
			records = [];
			this.#granted.set(path, records);
		}
		records.push(record);
		const combined = AbortSignal.any([signal, this.#abort.signal]);
		return new Promise((resolve) => {
			let settled = false;
			// A caller that gives up never ungrants, so the grant is given back once it lands.
			const abandon = (): void => {
				if (settled) return;
				settled = true;
				if (record.state === 'held') {
					this.#forget(path, record);
					this.#sendUngrant([path]);
				} else {
					record.released = true;
				}
				resolve(CANCELLED);
			};
			combined.addEventListener('abort', abandon, { once: true });
			void this.#ask(path, record, combined).then((result) => {
				combined.removeEventListener('abort', abandon);
				if (settled) return;
				settled = true;
				resolve(result);
			});
		});
	}

	/**
	 * Asks the Server for one grant, a new one or one a restart dropped, and settles its record;
	 * `signal` gives up only while it waits to be sent.
	 */
	async #ask(
		path: string,
		record: GrantRecord,
		signal?: AbortSignal,
	): Promise<FileResult<FileEntry>> {
		const regrant = record.state === 'lost';
		const restarts = this.#channel.restarts;
		record.state = 'pending';
		const result = await this.#channel.request(
			FILES_OPERATIONS['fs.grant'].encode({
				op: 'fs.grant',
				path,
				source: record.source,
				workspaceId: this.workspaceId,
			}),
			(answer) => FILES_OPERATIONS['fs.grant'].decode(answer),
			{ signal },
		);
		if (result.kind === 'ok') {
			if (record.released || this.#closed) {
				this.#forget(path, record);
				this.#sendUngrant([path]);
			} else if (this.#channel.restarts === restarts) {
				record.state = 'held';
			} else {
				// The process that granted it may be the one a restart ended, so it is asked again.
				record.state = 'lost';
				void this.#ask(path, record);
			}
			return result;
		}
		const { error } = result;
		const restarted =
			error.code === 'unavailable' &&
			(error.reason === 'restarting' || error.reason === 'crashed-repeatedly');
		// A grant the person holds is asked for again once the service answers; a new one just fails.
		if (regrant && restarted && !record.released && !this.#closed) record.state = 'lost';
		else this.#forget(path, record);
		return result;
	}

	#ungrant(paths: readonly string[]): void {
		const sent: string[] = [];
		for (const path of paths) {
			const records = this.#granted.get(path);
			if (records === undefined) continue;
			// Give back one grant: a held one first, else one a restart dropped, else one in flight.
			const record =
				records.find((candidate) => candidate.state === 'held') ??
				records.find((candidate) => candidate.state === 'lost') ??
				records.find((candidate) => !candidate.released);
			if (record === undefined) continue;
			if (record.state === 'pending') {
				record.released = true;
				continue;
			}
			if (record.state === 'held') sent.push(path);
			this.#forget(path, record);
		}
		this.#sendUngrant(sent);
	}

	#forget(path: string, record: GrantRecord): void {
		const records = this.#granted.get(path);
		if (records === undefined) return;
		const index = records.indexOf(record);
		if (index >= 0) records.splice(index, 1);
		if (records.length === 0) this.#granted.delete(path);
	}

	/** Gives grants back; this outlives the session, since the Server holds them until it hears. */
	#sendUngrant(paths: readonly string[]): void {
		for (const batch of pathBatches(paths)) {
			void this.#channel.request(
				FILES_OPERATIONS['fs.ungrant'].encode({
					op: 'fs.ungrant',
					paths: [...batch],
					workspaceId: this.workspaceId,
				}),
				(answer) => FILES_OPERATIONS['fs.ungrant'].decode(answer),
			);
		}
	}

	#repository(
		op: 'repository.close' | 'repository.consent',
		repository: string,
		signal: AbortSignal,
	): Promise<FileResult<null>> {
		const { workspaceId } = this;
		const record =
			op === 'repository.close'
				? FILES_OPERATIONS[op].encode({ op, repository, workspaceId })
				: FILES_OPERATIONS[op].encode({ op, repository, workspaceId });
		return this.#run(signal, (combined) =>
			this.#channel.request(record, (answer) => FILES_OPERATIONS[op].decode(answer), {
				signal: combined,
			}),
		);
	}
}
