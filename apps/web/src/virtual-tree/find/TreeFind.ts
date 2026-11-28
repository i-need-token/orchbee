import type { TreeViewport } from '../layout/TreeViewport';
import type {
	VirtualTreeEvent,
	VirtualTreeFindMatchType,
	VirtualTreeFindQuery,
	VirtualTreeRow,
} from '../model/virtual-tree-types';
import type {
	VirtualTreeFindMode,
	VirtualTreeFindProvider,
	VirtualTreeFindResult,
} from '../types/virtual-tree-find';
import { FindHistory } from './FindHistory';

/** How long typing rests before the provider is asked, as VS Code's asynchronous find waits. */
const PROVIDER_DELAY_MILLISECONDS = 250;

/** What the find shows under its input: that nothing matched, or a provider's warning. */
type TreeFindMessage =
	| { readonly kind: 'notFound' }
	| { readonly kind: 'warning'; readonly text: string };

/** One announcement of how many items a find found; a new serial asks to announce it again. */
interface TreeFindAnnouncement {
	readonly count: number;
	readonly serial: number;
}

export interface TreeFindSnapshot {
	readonly open: boolean;
	/** The input's text, which is the pattern the find applies. */
	readonly value: string;
	readonly mode: VirtualTreeFindMode;
	readonly matchType: VirtualTreeFindMatchType;
	readonly message: TreeFindMessage | null;
	/** The latest result to announce politely; null until a pattern has found something or nothing. */
	readonly announcement: TreeFindAnnouncement | null;
	/** Grows whenever the input should take focus and select its text. */
	readonly focusRequest: number;
}

export interface TreeFindOptions<K, T> {
	/** The text a row is found by; it keeps its identity, since a new one starts a find over. */
	readonly label: (row: VirtualTreeRow<K, T>) => string;
	/** The provider to ask now, read whenever the find asks one; null for none. */
	readonly provider: () => VirtualTreeFindProvider<K, T> | null;
	/** Moves keyboard focus to the tree, as closing the find or leaving its input does. */
	readonly focusTree: () => void;
}

const NOT_FOUND: TreeFindMessage = { kind: 'notFound' };

/**
 * VS Code's tree find controller over a viewport, without its widget: the find opens with the last
 * pattern it closed with, applies each pattern as it is typed, moves focus to the first matched row
 * from the focused one, says when nothing matched and announces how many items did. While a pattern
 * applies, keys that move focus land only on matched rows, unless at most one matched. Up and Down in
 * the input walk the patterns it remembers, as VS Code's history input box does.
 *
 * With a provider, the loaded rows are still found on every keystroke, and the provider is asked once
 * typing rests for 250 ms, one request at a time: its answer brings focus to a match again, sets the
 * message and the announcement, and, while filtering, lets its `isVisible` hide rows until the next
 * answer or a change of mode. A session starts with its first request and ends when the input
 * empties, or when the tree goes, once the provider is asked again.
 */
export class TreeFind<K, T> {
	private readonly viewport: TreeViewport<K, T>;
	private readonly options: TreeFindOptions<K, T>;
	private readonly history = new FindHistory();
	private readonly listeners = new Set<() => void>();
	private snapshot: TreeFindSnapshot;
	private previousPattern = '';
	private announcements = 0;
	/** The query last handed to the model; null for no find. */
	private requested: VirtualTreeFindQuery<K, T> | null = null;
	/**
	 * Whether focus moves to a matched row once the model applies the query asked for, as a new
	 * pattern moves it; null while the model shows it already.
	 */
	private pending: { readonly moveFocus: boolean } | null = null;
	private disconnect: (() => void) | null = null;
	// The provider's side, as VS Code's asynchronous find controller keeps it.
	private session = false;
	/** Whether the provider has yet to answer for the current pattern. */
	private waiting = false;
	/** The provider's answer for the current pattern. */
	private result: VirtualTreeFindResult<K, T> | null = null;
	/** The provider's `isVisible` since its latest answer while filtering; a new one settles the rows. */
	private visible: ((row: VirtualTreeRow<K, T>) => boolean) | null = null;
	private abort: AbortController | null = null;
	private timer: ReturnType<typeof setTimeout> | null = null;
	private step: Promise<void> | null = null;
	private stepQueued = false;

	constructor(viewport: TreeViewport<K, T>, options: TreeFindOptions<K, T>) {
		this.viewport = viewport;
		this.options = options;
		this.snapshot = {
			announcement: null,
			focusRequest: 0,
			// VS Code's defaults, until `setMode` and `setMatchType` choose others.
			matchType: 'fuzzy',
			message: null,
			mode: 'highlight',
			open: false,
			value: '',
		};
	}

	readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	readonly getSnapshot = (): TreeFindSnapshot => this.snapshot;

	/** Starts following the model, and returns what stops it, which also closes the find. */
	readonly connect = (): (() => void) => {
		this.disconnect?.();
		this.disconnect = this.viewport.model.subscribe(this.hear);
		return () => {
			this.disconnect?.();
			this.disconnect = null;
			this.cancelProvider();
			const provider = this.options.provider();
			if (this.session && provider !== null) {
				this.session = false;
				provider.endSession?.().catch(() => undefined);
			}
			this.result = null;
			this.visible = null;
			this.waiting = false;
			if (this.snapshot.open) this.previousPattern = this.snapshot.value;
			this.update({ message: null, open: false, value: '' });
			this.requested = null;
			this.pending = null;
			this.viewport.model.find(null);
		};
	};

	/**
	 * VS Code's `list.find`: opens the find with the pattern it last closed with, applied again, or,
	 * while it is open, selects its input's text.
	 */
	open(): void {
		const { focusRequest, open, value } = this.snapshot;
		if (open) {
			this.history.add(value, true);
			this.update({ focusRequest: focusRequest + 1 });
			return;
		}
		const pattern = this.previousPattern;
		this.history.restart();
		this.history.add(pattern, true);
		this.update({ focusRequest: focusRequest + 1, message: null, open: true, value: pattern });
		if (pattern !== '') this.findAgain(true);
	}

	/** VS Code's `list.closeFind`: every row shows again, and focus returns to the tree. */
	close(): void {
		const { open, value } = this.snapshot;
		if (!open) return;
		this.previousPattern = value;
		this.update({ message: null, open: false, value: '' });
		this.findAgain(true);
		this.options.focusTree();
	}

	/** Applies the input's new text. */
	type(value: string): void {
		if (!this.snapshot.open || value === this.snapshot.value) return;
		this.update({ value });
		this.findAgain(true);
	}

	/** Enter in the input: remembers the pattern and returns to the tree. */
	commit(): void {
		this.history.add(this.snapshot.value);
		this.options.focusTree();
	}

	/** Up in the input: the pattern before the one it shows, when it remembers one. */
	historyPrevious(): void {
		const previous = this.history.previous(this.snapshot.value);
		if (previous !== null) this.type(previous);
	}

	/** Down in the input: the pattern after the one it shows, or, past the newest, back to the tree. */
	historyNext(): void {
		if (this.history.atEnd) {
			this.commit();
			return;
		}
		this.type(this.history.next(this.snapshot.value));
	}

	setMode(mode: VirtualTreeFindMode): void {
		if (mode === this.snapshot.mode) return;
		// The provider's answer was for the other mode.
		this.visible = null;
		this.update({ mode });
		if (this.snapshot.open) this.findAgain(false);
	}

	setMatchType(matchType: VirtualTreeFindMatchType): void {
		if (matchType === this.snapshot.matchType) return;
		this.update({ matchType });
		if (this.snapshot.open) this.findAgain(false);
	}

	/**
	 * VS Code's focus navigation filter: whether keys that move focus may land on a row. Every row
	 * may until a pattern applies or while at most one row matched; otherwise only a matched row, or
	 * one the provider's answer calls a match.
	 */
	readonly allowsFocus = (row: VirtualTreeRow<K, T>): boolean => {
		const { open, value } = this.snapshot;
		if (!open || value === '') return true;
		const { model } = this.viewport;
		const result = this.result;
		if (result !== null) return result.isMatch(row) || model.matchesFind(row);
		const status = model.findStatus;
		if (status === null || (status.totalCount > 0 && status.matchCount <= 1)) return true;
		return model.matchesFind(row);
	};

	private readonly hear = (event: VirtualTreeEvent<K>): void => {
		if (event.kind !== 'find') return;
		if ((this.viewport.model.findStatus?.query ?? null) !== this.requested) return;
		if (this.pending !== null) this.settle();
		// The rows changed what matched; VS Code renders again without moving focus.
		else if (this.snapshot.open) this.render(false);
	};

	/**
	 * Finds the input's text again: at once among the loaded rows, and through the provider once
	 * typing rests, whose answer the message and the announcement then wait for.
	 */
	private findAgain(moveFocus: boolean): void {
		if (this.options.provider() !== null) {
			this.result = null;
			this.waiting = true;
			this.update({ message: null });
			this.scheduleProvider();
		}
		this.request(moveFocus);
	}

	private request(moveFocus: boolean): void {
		const { matchType, mode, value } = this.snapshot;
		const filter = mode === 'filter';
		const query =
			value === ''
				? null
				: {
						filter,
						label: this.options.label,
						matchType,
						pattern: value,
						visible: filter ? (this.visible ?? undefined) : undefined,
					};
		this.requested = query;
		this.pending = { moveFocus };
		const { model } = this.viewport;
		model.find(query);
		// The model sends no event when no find applied and none applies now.
		if ((model.findStatus?.query ?? null) === query) this.settle();
	}

	/**
	 * What VS Code's find does once the tree refiltered: move focus, reveal it, and render, which
	 * waits for a provider's answer.
	 */
	private settle(): void {
		const pending = this.pending;
		if (pending === null) return;
		this.pending = null;
		const { model } = this.viewport;
		if (pending.moveFocus) {
			if (this.requested !== null) this.focusFirstMatch();
			const focus = model.focus;
			// VS Code's reveal would also expand the rows above a focused row a collapse hides.
			if (focus !== null && model.indexOf(focus) >= 0 && !this.viewport.inSight(focus)) {
				model.reveal(focus, { relativeTop: 0.5 });
			}
		}
		this.render(true);
	}

	/** VS Code's `focusNext(0, true)` over matched rows: the focused row or the next match, wrapping. */
	private focusFirstMatch(): void {
		const { model } = this.viewport;
		if (model.findStatus?.matchCount === 0) return;
		const count = model.rowCount;
		const focus = model.focus === null ? -1 : model.indexOf(model.focus);
		const start = Math.max(focus, 0);
		for (let offset = 0; offset < count; offset += 1) {
			const row = model.rowAt((start + offset) % count);
			if (!model.matchesFind(row)) continue;
			model.setFocus(row.key);
			return;
		}
	}

	/** VS Code's `render`: the message, and, after a pattern applied, the polite announcement. */
	private render(announce: boolean): void {
		const { value } = this.snapshot;
		let count: number;
		let message: TreeFindMessage | null;
		if (this.options.provider() === null) {
			const status = this.viewport.model.findStatus;
			count = status?.matchCount ?? 0;
			const missed = status !== null && status.matchCount === 0 && status.totalCount > 0;
			message = missed && value !== '' ? NOT_FOUND : null;
		} else {
			const result = this.result;
			if (this.waiting || result === null) return;
			count = result.matchCount;
			message = count === 0 && value !== '' ? NOT_FOUND : null;
			if (result.warning !== undefined && result.warning !== '') {
				message = sameMessage(this.snapshot.message, result.warning)
					? this.snapshot.message
					: { kind: 'warning', text: result.warning };
			}
		}
		if (!announce || value === '') {
			this.update({ message });
			return;
		}
		this.announcements += 1;
		this.update({ announcement: { count, serial: this.announcements }, message });
	}

	private scheduleProvider(): void {
		this.abort?.abort();
		this.abort = new AbortController();
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = setTimeout(() => {
			this.timer = null;
			this.startStep();
		}, PROVIDER_DELAY_MILLISECONDS);
	}

	private cancelProvider(): void {
		this.abort?.abort();
		this.abort = null;
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
		this.stepQueued = false;
	}

	/** Runs the provider's step, or queues it behind the running one, as VS Code's throttler does. */
	private startStep(): void {
		if (this.step !== null) {
			this.stepQueued = true;
			return;
		}
		// A provider that throws has reported nothing; the next step runs all the same.
		this.step = this.runStep()
			.catch(() => undefined)
			.then(() => {
				this.step = null;
				if (!this.stepQueued) return;
				this.stepQueued = false;
				this.startStep();
			});
	}

	/** VS Code's `applyPatternAsync`, for the input's text as it is when the step runs. */
	private async runStep(): Promise<void> {
		const provider = this.options.provider();
		const signal = this.abort?.signal;
		if (provider === null || signal === undefined || signal.aborted) return;
		const { matchType, mode, value } = this.snapshot;
		if (value === '') {
			if (!this.session) return;
			this.session = false;
			this.result = null;
			this.visible = null;
			await provider.endSession?.().catch(() => undefined);
			return;
		}
		if (!this.session) {
			this.session = true;
			provider.startSession?.();
		}
		// A provider reports its own failures; a failed find leaves the rows as they are.
		const result = await provider.find(value, { matchType, mode }, signal).catch(() => undefined);
		if (signal.aborted || result === undefined) return;
		this.waiting = false;
		this.result = result;
		if (mode === 'filter' && provider.isVisible !== undefined) {
			this.visible = (row) => provider.isVisible?.(row) ?? true;
		}
		this.request(true);
	}

	private update(patch: Partial<TreeFindSnapshot>): void {
		const current = this.snapshot;
		const next: TreeFindSnapshot = { ...current, ...patch };
		if (
			next.open === current.open &&
			next.value === current.value &&
			next.mode === current.mode &&
			next.matchType === current.matchType &&
			next.message === current.message &&
			next.announcement === current.announcement &&
			next.focusRequest === current.focusRequest
		) {
			return;
		}
		this.snapshot = next;
		for (const listener of this.listeners) listener();
	}
}

function sameMessage(message: TreeFindMessage | null, warning: string): boolean {
	return message?.kind === 'warning' && message.text === warning;
}
