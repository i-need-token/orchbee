import type { FileResource } from '../../file-session';

/** How many closed tabs Reopen Closed Editor remembers, as VS Code's history does. */
const MAXIMUM_CLOSED_TABS = 20;

/** What a tab shows: so far always one file's text. */
interface FileTabInput {
	readonly kind: 'file';
	readonly resource: FileResource;
}

interface FileTab {
	/** Its kind with its resources' keys, so a file has one text tab and a diff tab may join it. */
	readonly id: string;
	readonly input: FileTabInput;
	/** Whether its content holds edits not yet saved. */
	readonly unsaved: boolean;
}

/** A closed tab that Reopen Closed Editor brings back pinned, where it was if that still fits. */
interface ClosedFileTab {
	readonly id: string;
	readonly index: number;
	readonly input: FileTabInput;
	readonly sticky: boolean;
}

/** A close that waits for the person's save decision on the unsaved tabs among those it takes. */
interface PendingFileTabClose {
	/** Every tab the close takes, in closing order. */
	readonly ids: readonly string[];
	/** Asking for the decision, or saving after Save All. */
	readonly phase: 'deciding' | 'saving';
}

/**
 * One Workspace's file tabs, as VS Code's single editor group keeps them: sticky tabs form a prefix
 * of the row, at most one tab is a preview and it is never sticky, and the active tab leads the
 * recent order, which holds every tab once.
 */
export interface FileTabs {
	readonly activeId: string | null;
	/** Batches of tabs closed together, oldest first: at most 20 tabs in all and none open. */
	readonly closed: readonly (readonly ClosedFileTab[])[];
	readonly pendingClose: PendingFileTabClose | null;
	readonly previewId: string | null;
	/** Every tab's id from the most to the least recently active. */
	readonly recent: readonly string[];
	/** How many tabs at the start of the row are sticky. */
	readonly stickyCount: number;
	/** The row, left to right. */
	readonly tabs: readonly FileTab[];
}

export const EMPTY_FILE_TABS: FileTabs = {
	activeId: null,
	closed: [],
	pendingClose: null,
	previewId: null,
	recent: [],
	stickyCount: 0,
	tabs: [],
};

/** Which tabs a close takes, as VS Code's close commands choose them. */
type FileTabCloseTarget =
	/**
	 * Close Editor from a key: the active tab, unless it is sticky, when the most recent non-sticky
	 * tab becomes active instead.
	 */
	| { readonly kind: 'active' }
	/** Close All: every tab but the sticky ones. */
	| { readonly kind: 'all' }
	/** Close Editors to the Left or to the Right of a tab, sticky ones excepted. */
	| { readonly id: string; readonly kind: 'left' | 'right' }
	/** Close Others: every non-sticky tab but these, which it pins. */
	| { readonly keepIds: readonly string[]; readonly kind: 'others' }
	/** Closing the overlay, as Close All Editor Groups does: every tab, sticky ones too. */
	| { readonly kind: 'overlay' }
	/** Close Saved: every non-sticky tab without unsaved edits. */
	| { readonly kind: 'saved' }
	/** Close from a tab's button or menu: these tabs, sticky or not. */
	| { readonly ids: readonly string[]; readonly kind: 'tabs' };

export type FileTabsEvent =
	/** The person's answer to the save question: Save All, Don't Save or Cancel. */
	| { readonly decision: 'cancel' | 'discard' | 'save'; readonly type: 'close-decided' }
	| { readonly target: FileTabCloseTarget; readonly type: 'close-requested' }
	/** Saving after Save All finished; the tabs that failed to save stay open. */
	| { readonly failedIds: readonly string[]; readonly type: 'close-saved' }
	/** A file opens from any source: a single click opens a preview, everything else a pinned tab. */
	| { readonly input: FileTabInput; readonly preview: boolean; readonly type: 'file-opened' }
	| { readonly type: 'reopen-requested' }
	| { readonly id: string; readonly type: 'tab-activated' }
	/** Keep Open, or a double-click, pins a preview tab. */
	| { readonly id: string; readonly type: 'tab-kept' }
	/** A drag in the row moves and pins a tab; crossing the sticky boundary changes its stickiness. */
	| { readonly id: string; readonly index: number; readonly type: 'tab-moved' }
	| { readonly id: string; readonly type: 'tab-stuck' }
	| { readonly id: string; readonly type: 'tab-unstuck' }
	/** The tab's content gained or lost unsaved edits; gaining them pins a preview tab. */
	| { readonly id: string; readonly type: 'unsaved-changed'; readonly unsaved: boolean };

/** What the save decision asks of the tabs' content; the outcome returns as a later event. */
export type FileTabsEffect =
	/** Drop these tabs' edits, so content that another tab still shows matches the disk again. */
	| { readonly kind: 'revert'; readonly tabs: readonly FileTab[] }
	/** Save these tabs, then report `close-saved` with the ones that failed. */
	| { readonly kind: 'save'; readonly tabs: readonly FileTab[] };

interface FileTabsTransition {
	readonly effects: readonly FileTabsEffect[];
	readonly tabs: FileTabs;
}

const NO_EFFECTS: readonly FileTabsEffect[] = [];

/** The state one transition works on; it settles into `FileTabs`, sharing what did not change. */
interface FileTabsDraft {
	activeId: string | null;
	closed: ClosedFileTab[][];
	pendingClose: PendingFileTabClose | null;
	previewId: string | null;
	recent: string[];
	stickyCount: number;
	tabs: FileTab[];
}

function fileTabId(input: FileTabInput): string {
	return `${input.kind}:${input.resource.key}`;
}

function draftOf(tabs: FileTabs): FileTabsDraft {
	return {
		activeId: tabs.activeId,
		closed: tabs.closed.map((batch) => [...batch]),
		pendingClose: tabs.pendingClose,
		previewId: tabs.previewId,
		recent: [...tabs.recent],
		stickyCount: tabs.stickyCount,
		tabs: [...tabs.tabs],
	};
}

function sameItems<Item>(left: readonly Item[], right: readonly Item[]): boolean {
	return left.length === right.length && left.every((item, index) => item === right[index]);
}

/** The draft as state, keeping the earlier state, or its arrays, wherever nothing changed. */
function settle(draft: FileTabsDraft, earlier: FileTabs): FileTabs {
	const tabs = sameItems(draft.tabs, earlier.tabs) ? earlier.tabs : draft.tabs;
	const recent = sameItems(draft.recent, earlier.recent) ? earlier.recent : draft.recent;
	const closed =
		draft.closed.length === earlier.closed.length &&
		draft.closed.every((batch, index) => sameItems(batch, earlier.closed[index] ?? []))
			? earlier.closed
			: draft.closed;
	if (
		tabs === earlier.tabs &&
		recent === earlier.recent &&
		closed === earlier.closed &&
		draft.activeId === earlier.activeId &&
		draft.pendingClose === earlier.pendingClose &&
		draft.previewId === earlier.previewId &&
		draft.stickyCount === earlier.stickyCount
	) {
		return earlier;
	}
	return {
		activeId: draft.activeId,
		closed,
		pendingClose: draft.pendingClose,
		previewId: draft.previewId,
		recent,
		stickyCount: draft.stickyCount,
		tabs,
	};
}

function indexOfTab(draft: FileTabsDraft, id: string): number {
	return draft.tabs.findIndex((tab) => tab.id === id);
}

function isStickyIndex(draft: FileTabsDraft, index: number): boolean {
	return index >= 0 && index < draft.stickyCount;
}

function pin(draft: FileTabsDraft, id: string): void {
	if (draft.previewId === id) draft.previewId = null;
}

function activate(draft: FileTabsDraft, id: string): void {
	draft.activeId = id;
	const at = draft.recent.indexOf(id);
	if (at <= 0) return;
	draft.recent.splice(at, 1);
	draft.recent.unshift(id);
}

/** Adds a tab to the row; in the recent order it follows the active tab until it becomes active. */
function insert(draft: FileTabsDraft, index: number, tab: FileTab): void {
	draft.tabs.splice(Math.min(index, draft.tabs.length), 0, tab);
	draft.recent.splice(Math.min(1, draft.recent.length), 0, tab.id);
}

/**
 * Takes a tab out of the row and out of a waiting close. Removing the active tab activates the most
 * recent other one, unless the caller activates another itself; the removed tab's place is returned
 * for Reopen Closed Editor.
 */
function remove(draft: FileTabsDraft, id: string, pickNext: boolean): ClosedFileTab | null {
	const index = indexOfTab(draft, id);
	const tab = draft.tabs[index];
	if (tab === undefined) return null;
	const sticky = isStickyIndex(draft, index);
	if (draft.activeId === id) {
		const next = pickNext ? draft.recent[1] : undefined;
		if (next === undefined) draft.activeId = null;
		else activate(draft, next);
	}
	pin(draft, id);
	draft.tabs.splice(index, 1);
	if (sticky) draft.stickyCount -= 1;
	draft.recent.splice(draft.recent.indexOf(id), 1);
	const pending = draft.pendingClose;
	if (pending?.ids.includes(id) === true) {
		draft.pendingClose = { ...pending, ids: pending.ids.filter((other) => other !== id) };
	}
	return { id, index, input: tab.input, sticky };
}

/** Moves a tab within the row, which makes it sticky or not by the side of the boundary it lands. */
function move(draft: FileTabsDraft, id: string, toIndex: number): boolean {
	const to = Math.max(0, Math.min(toIndex, draft.tabs.length - 1));
	const from = indexOfTab(draft, id);
	const tab = draft.tabs[from];
	if (tab === undefined || from === to) return false;
	if (isStickyIndex(draft, from) && to >= draft.stickyCount) draft.stickyCount -= 1;
	else if (!isStickyIndex(draft, from) && to < draft.stickyCount) draft.stickyCount += 1;
	draft.tabs.splice(from, 1);
	draft.tabs.splice(to, 0, tab);
	return true;
}

function stick(draft: FileTabsDraft, id: string): void {
	const index = indexOfTab(draft, id);
	if (index < 0 || isStickyIndex(draft, index)) return;
	pin(draft, id);
	move(draft, id, draft.stickyCount);
	draft.stickyCount += 1;
}

function unstick(draft: FileTabsDraft, id: string): void {
	if (!isStickyIndex(draft, indexOfTab(draft, id))) return;
	move(draft, id, draft.stickyCount - 1);
	draft.stickyCount -= 1;
}

/** Removes a tab from Reopen Closed Editor's batches, as opening it again does. */
function forget(draft: FileTabsDraft, id: string): void {
	draft.closed = draft.closed
		.map((batch) => batch.filter((entry) => entry.id !== id))
		.filter((batch) => batch.length > 0);
}

/** Records one close's tabs as a batch that Reopen Closed Editor brings back together. */
function remember(draft: FileTabsDraft, batch: readonly ClosedFileTab[]): void {
	if (batch.length === 0) return;
	draft.closed.push([...batch]);
	let excess = draft.closed.reduce((count, entries) => count + entries.length, 0);
	excess -= MAXIMUM_CLOSED_TABS;
	while (excess > 0) {
		const oldest = draft.closed[0];
		if (oldest === undefined) break;
		const dropped = Math.min(excess, oldest.length);
		oldest.splice(0, dropped);
		if (oldest.length === 0) draft.closed.shift();
		excess -= dropped;
	}
}

interface OpenOptions {
	/** Where the tab goes, or null for right of the active tab. */
	readonly index: number | null;
	readonly pinned: boolean;
	readonly sticky: boolean;
}

/**
 * Opens a tab and activates it, as VS Code's group opens an editor with `openPositioning` right: a
 * new tab goes right of the active one and after the sticky tabs, and a new preview replaces the
 * old preview, which counts as replaced rather than closed. An open tab is activated, and pinned or
 * made sticky when asked; opening never makes a pinned tab a preview.
 */
function open(draft: FileTabsDraft, input: FileTabInput, options: OpenOptions): void {
	const id = fileTabId(input);
	forget(draft, id);
	const sticky = options.sticky || (options.index !== null && isStickyIndex(draft, options.index));
	const pinned = options.pinned || sticky;
	if (indexOfTab(draft, id) >= 0) {
		if (pinned) pin(draft, id);
		activate(draft, id);
		if (options.index !== null) move(draft, id, options.index);
		if (sticky) stick(draft, id);
		return;
	}
	let target: number;
	if (options.index !== null) {
		target = options.index;
	} else {
		target = (draft.activeId === null ? -1 : indexOfTab(draft, draft.activeId)) + 1;
		if (!sticky && isStickyIndex(draft, target)) target = draft.stickyCount;
	}
	if (sticky) {
		draft.stickyCount += 1;
		if (!isStickyIndex(draft, target)) target = draft.stickyCount - 1;
	}
	const tab: FileTab = { id, input, unsaved: false };
	const replaced = pinned ? null : draft.previewId;
	if (replaced === null) {
		insert(draft, target, tab);
	} else {
		if (target > indexOfTab(draft, replaced)) target -= 1;
		remove(draft, replaced, false);
		insert(draft, target, tab);
	}
	if (!pinned) draft.previewId = id;
	activate(draft, id);
}

/**
 * Closes tabs in the order given, inactive ones first and the active one last, so the most recent
 * remaining tab becomes active. They form one batch for Reopen Closed Editor.
 */
function close(draft: FileTabsDraft, ids: readonly string[]): void {
	const batch: ClosedFileTab[] = [];
	let closesActive = false;
	for (const id of new Set(ids)) {
		if (id === draft.activeId) {
			closesActive = true;
			continue;
		}
		const entry = remove(draft, id, true);
		if (entry !== null) batch.push(entry);
	}
	if (closesActive && draft.activeId !== null) {
		const entry = remove(draft, draft.activeId, true);
		if (entry !== null) batch.push(entry);
	}
	remember(draft, batch);
}

/**
 * The tabs a close takes, in VS Code's closing order: as given, in row order, or, for Close Saved,
 * from the most recent. Close Others pins the tabs it keeps and Close to the Right pins the active
 * tab before closing, as VS Code's commands do.
 */
function chooseClosing(draft: FileTabsDraft, target: FileTabCloseTarget): readonly string[] {
	const nonSticky = draft.tabs.slice(draft.stickyCount).map((tab) => tab.id);
	switch (target.kind) {
		case 'tabs':
			return target.ids;
		case 'active': {
			const activeId = draft.activeId;
			if (activeId === null) return [];
			if (!isStickyIndex(draft, indexOfTab(draft, activeId))) return [activeId];
			const next = draft.recent.find((id) => !isStickyIndex(draft, indexOfTab(draft, id)));
			if (next !== undefined) activate(draft, next);
			return [];
		}
		case 'others': {
			const kept = new Set(target.keepIds);
			for (const id of kept) pin(draft, id);
			return nonSticky.filter((id) => !kept.has(id));
		}
		case 'left':
		case 'right': {
			if (target.kind === 'right' && draft.activeId !== null) pin(draft, draft.activeId);
			const index = indexOfTab(draft, target.id);
			if (index < 0) return [];
			return draft.tabs
				.filter(
					(_tab, at) =>
						at >= draft.stickyCount && (target.kind === 'left' ? at < index : at > index),
				)
				.map((tab) => tab.id);
		}
		case 'saved':
			return draft.recent.filter(
				(id) => nonSticky.includes(id) && draft.tabs[indexOfTab(draft, id)]?.unsaved === false,
			);
		case 'all':
			return nonSticky;
		case 'overlay':
			return draft.tabs.map((tab) => tab.id);
	}
}

function unsavedTabs(draft: FileTabsDraft, ids: readonly string[]): readonly FileTab[] {
	return draft.tabs.filter((tab) => tab.unsaved && ids.includes(tab.id));
}

/**
 * A close of tabs without unsaved edits happens at once. Otherwise the first unsaved tab activates
 * so the person sees it behind the save question, and nothing closes until the decision: Cancel
 * keeps every tab, Don't Save closes them all after dropping the edits, and Save All closes the ones
 * that saved. One close waits at a time.
 */
function requestClose(draft: FileTabsDraft, target: FileTabCloseTarget): void {
	const ids = [...new Set(chooseClosing(draft, target))].filter((id) => indexOfTab(draft, id) >= 0);
	if (ids.length === 0) return;
	const unsaved = unsavedTabs(draft, ids);
	const first = ids.find((id) => unsaved.some((tab) => tab.id === id));
	if (first === undefined) {
		close(draft, ids);
		return;
	}
	activate(draft, first);
	draft.pendingClose = { ids, phase: 'deciding' };
}

/**
 * Reopens the last batch of closed tabs pinned, each at its old place when that keeps its
 * stickiness, and activates the tab that closed last, as VS Code's batch reopen ends. They reopen
 * last closed first, so each place is the one recorded when it closed and the row comes back as it
 * was. Opening a tab drops it from the batches, so every remembered tab is closed.
 */
function reopen(draft: FileTabsDraft): void {
	const batch = draft.closed.pop() ?? [];
	for (let at = batch.length - 1; at >= 0; at -= 1) {
		const entry = batch[at];
		if (entry === undefined) continue;
		const fits = entry.sticky === isStickyIndex(draft, entry.index);
		open(draft, entry.input, {
			index: fits ? entry.index : null,
			pinned: true,
			sticky: entry.sticky,
		});
	}
	const last = batch.at(-1);
	if (last !== undefined) activate(draft, last.id);
}

function decide(
	draft: FileTabsDraft,
	pending: PendingFileTabClose,
	decision: 'cancel' | 'discard' | 'save',
): readonly FileTabsEffect[] {
	const unsaved = unsavedTabs(draft, pending.ids);
	if (decision === 'cancel') {
		draft.pendingClose = null;
		return NO_EFFECTS;
	}
	if (decision === 'save' && unsaved.length > 0) {
		draft.pendingClose = { ...pending, phase: 'saving' };
		return [{ kind: 'save', tabs: unsaved }];
	}
	draft.pendingClose = null;
	close(draft, pending.ids);
	return unsaved.length > 0 ? [{ kind: 'revert', tabs: unsaved }] : NO_EFFECTS;
}

/**
 * The tab model's transitions. Events about tabs that are not open, decisions without a waiting
 * close and requests that change nothing leave the state as it was, the same object.
 */
export function transitionFileTabs(tabs: FileTabs, event: FileTabsEvent): FileTabsTransition {
	const draft = draftOf(tabs);
	let effects = NO_EFFECTS;
	switch (event.type) {
		case 'file-opened':
			open(draft, event.input, { index: null, pinned: !event.preview, sticky: false });
			break;
		case 'reopen-requested':
			reopen(draft);
			break;
		case 'tab-activated':
			if (indexOfTab(draft, event.id) >= 0) activate(draft, event.id);
			break;
		case 'tab-kept':
			pin(draft, event.id);
			break;
		case 'tab-moved':
			if (move(draft, event.id, event.index)) pin(draft, event.id);
			break;
		case 'tab-stuck':
			stick(draft, event.id);
			break;
		case 'tab-unstuck':
			unstick(draft, event.id);
			break;
		case 'unsaved-changed': {
			const index = indexOfTab(draft, event.id);
			const tab = draft.tabs[index];
			if (tab === undefined || tab.unsaved === event.unsaved) break;
			draft.tabs[index] = { ...tab, unsaved: event.unsaved };
			if (event.unsaved) pin(draft, event.id);
			break;
		}
		case 'close-requested':
			if (draft.pendingClose === null) requestClose(draft, event.target);
			break;
		case 'close-decided':
			if (draft.pendingClose?.phase === 'deciding') {
				effects = decide(draft, draft.pendingClose, event.decision);
			}
			break;
		case 'close-saved': {
			const pending = draft.pendingClose;
			if (pending?.phase !== 'saving') break;
			const failed = new Set(event.failedIds);
			draft.pendingClose = null;
			close(
				draft,
				pending.ids.filter((id) => !failed.has(id)),
			);
			break;
		}
	}
	return { effects, tabs: settle(draft, tabs) };
}
