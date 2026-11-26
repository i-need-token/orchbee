import type { WorkbenchWorkspaceFilesSource, WorkspaceSearch } from '../workbench';
import {
	describeMatch,
	type FileNameMatch,
	type FileQuery,
	lowerAligned,
	mayMatch,
	narrows,
	parseFileQuery,
	scorePath,
} from './file-name-match';

/** What a query found: the best paths, best first, and the line and column it named. */
export interface FileFinderResult {
	readonly matches: readonly FileNameMatch[];
	readonly line?: number | undefined;
	readonly column?: number | undefined;
}

/** How the listing stands: still arriving, whole, cut at its limit, or failed. */
type ListingState = 'listing' | 'complete' | 'limit-reached' | 'failed';

/** How often paths still arriving ask a shown search to run again. */
const ARRIVAL_NOTICE_MS = 150;

/** What the last query kept, so a longer one searches only those and the paths since. */
interface Narrowing {
	readonly query: FileQuery;
	readonly candidates: readonly number[];
	readonly checked: number;
}

/**
 * One Workspace's file names for quick open: listed once, the way a search reads the Workspace,
 * and matched in memory as the person types. Whoever made it closes it when quick open closes, so
 * nothing stays listed while it is not shown.
 */
export class WorkspaceFileFinder {
	private readonly paths: string[] = [];
	private readonly lowered: string[] = [];
	private readonly listeners = new Set<() => void>();
	private waiters: (() => void)[] = [];
	private state: ListingState = 'listing';
	private notice: ReturnType<typeof setTimeout> | null = null;
	private narrowing: Narrowing | null = null;
	private readonly listing: WorkspaceSearch;
	private closed = false;

	public constructor(source: WorkbenchWorkspaceFilesSource, workspaceId: string) {
		this.listing = source.listFiles(
			workspaceId,
			{ excludes: [], useIgnoreFiles: true },
			(report) => {
				if (this.closed) return;
				for (const path of report.paths) {
					this.paths.push(path);
					this.lowered.push(lowerAligned(path));
				}
				if (report.state !== 'searching') {
					this.state =
						report.state === 'complete' || report.state === 'limit-reached'
							? report.state
							: 'failed';
				}
				if (this.paths.length > 0 || this.state !== 'listing') this.wake();
				// A listing that failed with nothing found wakes the search waiting on it, which shows
				// the failure; telling the listeners too would run that search again, and a new
				// listing with it, for as long as the listing keeps failing.
				if (!this.failed) this.announce(this.state !== 'listing');
			},
		);
	}

	/** Whether the listing failed before it found anything, which a retry starts over. */
	public get failed(): boolean {
		return this.state === 'failed' && this.paths.length === 0;
	}

	/** Settles once there is something to match, the listing ended, or `signal` aborts. */
	public ready(signal: AbortSignal): Promise<void> {
		if (this.paths.length > 0 || this.state !== 'listing' || signal.aborted) {
			return Promise.resolve();
		}
		return new Promise((resolve) => {
			this.waiters.push(resolve);
			signal.addEventListener('abort', () => resolve(), { once: true });
		});
	}

	/** Tells `listener` when more paths arrived or the listing ended. */
	public subscribe(listener: () => void): () => void {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** The paths that best answer `text`, at most `limit`, best first. */
	public find(text: string, limit: number): FileFinderResult {
		const query = parseFileQuery(text);
		const location = {
			...(query.line === undefined ? {} : { line: query.line }),
			...(query.column === undefined ? {} : { column: query.column }),
		};
		if (query.pieces.length === 0) {
			this.narrowing = null;
			return { matches: [], ...location };
		}
		const candidates: number[] = [];
		const previous = this.narrowing;
		const from = previous !== null && narrows(previous.query, query) ? previous : null;
		const consider = (index: number): void => {
			const lowered = this.lowered[index];
			if (lowered !== undefined && mayMatch(query, lowered)) candidates.push(index);
		};
		if (from === null) {
			for (let index = 0; index < this.paths.length; index += 1) consider(index);
		} else {
			for (const index of from.candidates) consider(index);
			for (let index = from.checked; index < this.paths.length; index += 1) consider(index);
		}
		this.narrowing = { candidates, checked: this.paths.length, query };

		const scored: { readonly index: number; readonly score: number }[] = [];
		for (const index of candidates) {
			const path = this.paths[index];
			const lowered = this.lowered[index];
			if (path === undefined || lowered === undefined) continue;
			const score = scorePath(query, path, lowered);
			if (score !== null) scored.push({ index, score });
		}
		scored.sort((left, right) => {
			if (left.score !== right.score) return right.score - left.score;
			const leftPath = this.paths[left.index] ?? '';
			const rightPath = this.paths[right.index] ?? '';
			if (leftPath.length !== rightPath.length) return leftPath.length - rightPath.length;
			return leftPath < rightPath ? -1 : Number(leftPath > rightPath);
		});
		const matches = scored.slice(0, limit).flatMap(({ index }) => {
			const path = this.paths[index];
			const lowered = this.lowered[index];
			return path === undefined || lowered === undefined
				? []
				: [describeMatch(query, path, lowered)];
		});
		return { matches, ...location };
	}

	/** Stops the listing if it still runs and lets go of what it found. */
	public close(): void {
		if (this.closed) return;
		this.closed = true;
		this.listing.close();
		if (this.notice !== null) clearTimeout(this.notice);
		this.notice = null;
		this.listeners.clear();
		this.wake();
		this.paths.length = 0;
		this.lowered.length = 0;
		this.narrowing = null;
	}

	private wake(): void {
		const waiters = this.waiters;
		this.waiters = [];
		for (const resolve of waiters) resolve();
	}

	/** Tells the listeners, at once when the listing ended and otherwise at most every notice. */
	private announce(now: boolean): void {
		if (now) {
			if (this.notice !== null) clearTimeout(this.notice);
			this.notice = null;
			this.emit();
			return;
		}
		if (this.notice !== null) return;
		this.notice = setTimeout(() => {
			this.notice = null;
			this.emit();
		}, ARRIVAL_NOTICE_MS);
	}

	private emit(): void {
		// A copy, so a listener that subscribes or unsubscribes while this loops is not run twice.
		for (const listener of Array.from(this.listeners)) listener();
	}
}
