import { pathBatches } from './server-file-exchange';

/** What the connection's Server holds of one folder of one Workspace. */
interface FolderLoad {
	/** Sessions that hold the folder loaded. */
	holders: number;
	/** Listings of the folder in flight, any of which may load it again. */
	listings: number;
	/** A listing loaded the folder, and no release has gone out since. */
	loaded: boolean;
	/** The release in flight, which a listing of the folder waits out. */
	releasing: Promise<void> | null;
}

/** Sends one `fs.release`, settling once it was answered or failed. */
type SendRelease = (workspaceId: string, paths: readonly string[]) => Promise<void>;

function folderKey(workspaceId: string, path: string): string {
	return `${workspaceId}\u0000${path}`;
}

/**
 * The folders a connection's listings loaded, which the root stream reports changes in. The Server
 * keeps one load per folder for the connection, so this releases a folder once no session holds it
 * and no listing of it is in flight, and a listing waits out a release of its folder: the service
 * runs each request on its own thread, so a listing sent behind a release could otherwise run
 * first and leave the folder unloaded. A folder that a single-child listing of its parent loads is
 * not waited for, so a release of it already in flight may still unload it.
 */
export class ServerFileLoads {
	readonly #folders = new Map<string, FolderLoad>();
	/** Folders to release at the next flush, by Workspace. */
	readonly #queued = new Map<string, Set<string>>();
	#flushQueued = false;
	readonly #sendRelease: SendRelease;

	constructor(sendRelease: SendRelease) {
		this.#sendRelease = sendRelease;
	}

	/** Waits out a release of the folder in flight, then counts a listing of it until `listed`. */
	async list(workspaceId: string, path: string): Promise<void> {
		for (;;) {
			const releasing = this.#folders.get(folderKey(workspaceId, path))?.releasing ?? null;
			if (releasing === null) break;
			await releasing;
		}
		this.#folder(workspaceId, path).listings += 1;
	}

	/**
	 * Ends a listing of `path`; `loaded` names the folders the Server loaded when it succeeded. The
	 * caller takes its holds, then settles each of them and `path`.
	 */
	listed(workspaceId: string, path: string, loaded: readonly string[]): void {
		this.#folder(workspaceId, path).listings -= 1;
		for (const folder of loaded) this.#folder(workspaceId, folder).loaded = true;
	}

	hold(workspaceId: string, path: string): void {
		this.#folder(workspaceId, path).holders += 1;
	}

	drop(workspaceId: string, path: string): void {
		const folder = this.#folders.get(folderKey(workspaceId, path));
		if (folder === undefined) return;
		folder.holders -= 1;
		this.settle(workspaceId, path);
	}

	/** Releases the folder once nothing holds or lists it, and forgets one the Server never loaded. */
	settle(workspaceId: string, path: string): void {
		const key = folderKey(workspaceId, path);
		const folder = this.#folders.get(key);
		if (folder === undefined || folder.holders > 0 || folder.listings > 0) return;
		if (folder.releasing !== null) return;
		if (!folder.loaded) {
			this.#folders.delete(key);
			return;
		}
		let queued = this.#queued.get(workspaceId);
		if (queued === undefined) {
			queued = new Set();
			this.#queued.set(workspaceId, queued);
		}
		queued.add(path);
		if (this.#flushQueued) return;
		this.#flushQueued = true;
		// One release carries every folder a gesture let go, such as collapsing a tree.
		queueMicrotask(() => this.#flush());
	}

	#folder(workspaceId: string, path: string): FolderLoad {
		const key = folderKey(workspaceId, path);
		let folder = this.#folders.get(key);
		if (folder === undefined) {
			folder = { holders: 0, listings: 0, loaded: false, releasing: null };
			this.#folders.set(key, folder);
		}
		return folder;
	}

	#flush(): void {
		this.#flushQueued = false;
		for (const [workspaceId, paths] of this.#queued) {
			this.#queued.delete(workspaceId);
			const due = [...paths].filter((path) => {
				const folder = this.#folders.get(folderKey(workspaceId, path));
				// A listing or hold that began since it was queued keeps the folder.
				return (
					folder !== undefined &&
					folder.loaded &&
					folder.holders === 0 &&
					folder.listings === 0 &&
					folder.releasing === null
				);
			});
			for (const batch of pathBatches(due)) this.#release(workspaceId, batch);
		}
	}

	#release(workspaceId: string, paths: readonly string[]): void {
		const releasing = this.#sendRelease(workspaceId, paths);
		for (const path of paths) {
			const folder = this.#folder(workspaceId, path);
			folder.loaded = false;
			folder.releasing = releasing;
		}
		void releasing.then(() => {
			for (const path of paths) {
				const folder = this.#folders.get(folderKey(workspaceId, path));
				if (folder?.releasing !== releasing) continue;
				folder.releasing = null;
				this.settle(workspaceId, path);
			}
		});
	}
}
