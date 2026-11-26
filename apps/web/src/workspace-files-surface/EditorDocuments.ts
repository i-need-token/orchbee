import {
	MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES,
	WORKSPACE_FILES_WRITE_REQUEST_BYTES,
	type WorkspaceFileVersion,
	type WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';
import type * as MonacoApi from 'monaco-editor/editor/editor.api';

import type { WorkbenchWorkspaceFilesSource } from '../workbench';
import { readFileChunks } from './readFileChunks';

/** Room the save request's other fields take beside the escaped text. */
const REQUEST_ENVELOPE_BYTES = 8 * 1024;

const utf8 = new TextEncoder();

type FailedReason = 'denied' | 'not-found' | 'unavailable' | 'unconfirmed';

/** Why a document's edits are not on disk, shown above its editor until it is resolved. */
export type DocumentNotice =
	/** The file changed on disk since it was read, or is gone when `version` is null. */
	| { readonly kind: 'conflict'; readonly version: WorkspaceFileVersion | null }
	/**
	 * The save failed: the file or its folder refused it, the folder is gone, the Server could not
	 * save, or no answer came, which leaves unknown whether it saved.
	 */
	| { readonly kind: 'failed'; readonly reason: FailedReason }
	/** The text is larger than a save carries. */
	| { readonly kind: 'too-large' };

/** What the float window shows of one working-tree file it edits. */
export interface DocumentState {
	/** The text differs from the version it was read or saved at. */
	readonly dirty: boolean;
	/** The file was read whole with its hash, so a save can check it is still that version. */
	readonly editable: boolean;
	readonly saving: boolean;
	readonly notice: DocumentNotice | null;
}

/** How a save ended for the one who asked for it. */
export type SaveResult = 'written' | 'conflict' | 'failed' | 'skipped';

/** The part of a Monaco text model the holder uses; any text model is one. */
export interface DocumentModel {
	dispose(): void;
	getAlternativeVersionId(): number;
	getPositionAt(offset: number): MonacoApi.IPosition;
	getValue(): string;
	onDidChangeContent(listener: () => void): MonacoApi.IDisposable;
	pushEditOperations(
		before: MonacoApi.Selection[] | null,
		operations: MonacoApi.editor.IIdentifiedSingleEditOperation[],
		cursor: () => null,
	): unknown;
	pushStackElement(): void;
}

interface Entry<Model extends DocumentModel> {
	readonly model: Model;
	/** The disk version the saved text matches; null for a file read without its hash. */
	base: WorkspaceFileVersion | null;
	/** The model's alternative version id when its text matched `base`. */
	saved: number;
	saving: boolean;
	notice: DocumentNotice | null;
	/** The clock when a save or reload last touched the text; an older read must not land. */
	touched: number;
	/** The clock of the newest read taken, so an older one, such as another tab's, never lands. */
	seen: number;
	/** True while the holder itself replaces the text, which is never an edit. */
	applying: boolean;
	state: DocumentState;
	readonly listener: MonacoApi.IDisposable;
}

function keyOf(workspaceId: string, path: string): string {
	return `${workspaceId}\u0000${path}`;
}

function snapshotOf(entry: Entry<DocumentModel>): DocumentState {
	return {
		dirty: entry.model.getAlternativeVersionId() !== entry.saved,
		editable: typeof entry.base?.sha256 === 'string',
		notice: entry.notice,
		saving: entry.saving,
	};
}

/** Whether `offset` falls inside a surrogate pair or between the two characters of a CRLF. */
function splitsUnit(text: string, offset: number): boolean {
	const before = text.charCodeAt(offset - 1);
	const after = text.charCodeAt(offset);
	const pair = before >= 0xd8_00 && before <= 0xdb_ff && after >= 0xdc_00 && after <= 0xdf_ff;
	return pair || (before === 0x0d && after === 0x0a);
}

/**
 * Replaces a model's text with one undoable edit that spans only what changed, so the cursor and
 * scroll stay where they were and Undo brings the earlier text back, as VS Code does on a reload.
 */
function replaceText(model: DocumentModel, text: string): void {
	const current = model.getValue();
	if (current === text) return;
	let start = 0;
	const shorter = Math.min(current.length, text.length);
	while (start < shorter && current.charCodeAt(start) === text.charCodeAt(start)) start += 1;
	// Never split a surrogate pair or a CRLF, which the model treats as one unit and would widen.
	if (start > 0 && splitsUnit(current, start)) start -= 1;
	let end = 0;
	while (
		end < shorter - start &&
		current.charCodeAt(current.length - 1 - end) === text.charCodeAt(text.length - 1 - end)
	) {
		end += 1;
	}
	if (end > 0 && splitsUnit(current, current.length - end)) end -= 1;
	const from = model.getPositionAt(start);
	const to = model.getPositionAt(current.length - end);
	model.pushStackElement();
	model.pushEditOperations(
		[],
		[
			{
				range: {
					endColumn: to.column,
					endLineNumber: to.lineNumber,
					startColumn: from.column,
					startLineNumber: from.lineNumber,
				},
				text: text.slice(start, text.length - end),
			},
		],
		() => null,
	);
	model.pushStackElement();
}

/** Whether a text fits a save: the editable size, and the request once JSON escapes it. */
function fitsSave(text: string): boolean {
	return (
		utf8.encode(text).byteLength <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES &&
		utf8.encode(JSON.stringify(text)).byteLength + REQUEST_ENVELOPE_BYTES <=
			WORKSPACE_FILES_WRITE_REQUEST_BYTES
	);
}

/** What a save that did not land tells the person: no answer at all may still have landed. */
function failureOf(outcome: WorkspaceFilesOutcome | null): FailedReason {
	if (outcome?.kind !== 'failed') return 'unconfirmed';
	if (outcome.reason === 'denied' || outcome.reason === 'not-found') return outcome.reason;
	return 'unavailable';
}

/**
 * The working-tree files the float window shows, one text model per Workspace and path, as VS Code
 * keeps one model per resource: a file's tab and a diff against it edit the same text. The holder
 * outlives the editor pane, so edits survive collapsing the window and switching Workspaces, and it
 * saves a file only over the version it was read at. Models are made by the pane, which loads
 * Monaco; the holder only keeps and releases them.
 */
export class EditorDocuments<Model extends DocumentModel = MonacoApi.editor.ITextModel> {
	readonly #entries = new Map<string, Entry<Model>>();
	readonly #listeners = new Set<() => void>();
	#clock = 0;
	#version = 0;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** Moves whenever a document's state does, for `useSyncExternalStore`. */
	readonly getVersion = (): number => this.#version;

	/** Stamps a read as it starts, so a read older than a save, a reload or a read taken does not land. */
	now(): number {
		this.#clock += 1;
		return this.#clock;
	}

	state(workspaceId: string, path: string): DocumentState | null {
		return this.#entries.get(keyOf(workspaceId, path))?.state ?? null;
	}

	model(workspaceId: string, path: string): Model | null {
		return this.#entries.get(keyOf(workspaceId, path))?.model ?? null;
	}

	/** Every Workspace's paths whose edits are not saved; Workspaces with none are left out. */
	dirtyByWorkspace(): ReadonlyMap<string, ReadonlySet<string>> {
		const byWorkspace = new Map<string, Set<string>>();
		for (const [key, entry] of this.#entries) {
			if (!entry.state.dirty) continue;
			const split = key.indexOf('\u0000');
			const workspaceId = key.slice(0, split);
			const paths = byWorkspace.get(workspaceId) ?? new Set<string>();
			paths.add(key.slice(split + 1));
			byWorkspace.set(workspaceId, paths);
		}
		return byWorkspace;
	}

	/** The paths in a Workspace whose edits are not saved. */
	dirtyPaths(workspaceId: string): ReadonlySet<string> {
		const prefix = keyOf(workspaceId, '');
		const paths = new Set<string>();
		for (const [key, entry] of this.#entries) {
			if (key.startsWith(prefix) && entry.state.dirty) paths.add(key.slice(prefix.length));
		}
		return paths;
	}

	/**
	 * Takes a read of the file: the model for its text, made with `create` the first time. A read
	 * that started before the latest save or reload, or lands during a save, changes nothing. A file
	 * without edits, or whose edits match the text read, takes it; one with other edits keeps them,
	 * and a newer file on disk is a conflict the person resolves.
	 */
	adopt(
		workspaceId: string,
		path: string,
		text: string,
		version: WorkspaceFileVersion | null,
		readAt: number,
		create: (text: string) => Model,
	): Model {
		const key = keyOf(workspaceId, path);
		const existing = this.#entries.get(key);
		if (existing === undefined) {
			const model = create(text);
			const entry: Entry<Model> = {
				applying: false,
				base: version,
				listener: model.onDidChangeContent(() => {
					if (!entry.applying) this.#refresh(entry, false);
				}),
				model,
				notice: null,
				saved: model.getAlternativeVersionId(),
				saving: false,
				seen: readAt,
				state: { dirty: false, editable: false, notice: null, saving: false },
				touched: 0,
			};
			this.#entries.set(key, entry);
			this.#refresh(entry, true);
			return model;
		}
		if (existing.saving || readAt < existing.touched || readAt < existing.seen) {
			return existing.model;
		}
		existing.seen = readAt;
		// Edits the disk already holds, such as a save whose answer never came, are saved.
		if (!existing.state.dirty || text === existing.model.getValue()) {
			this.#apply(existing, text, version);
		} else if (version?.sha256 === null || version?.sha256 !== existing.base?.sha256) {
			const known = existing.notice?.kind === 'conflict' ? existing.notice.version : undefined;
			if (known === undefined || known?.sha256 !== version?.sha256) {
				existing.notice = { kind: 'conflict', version };
				this.#refresh(existing, true);
			}
		}
		return existing.model;
	}

	/**
	 * Takes a read that found no file. Edits stay, as a conflict whose save creates the file again;
	 * the model is returned so they keep showing. A file without edits answers null.
	 */
	gone(workspaceId: string, path: string, readAt: number): Model | null {
		const entry = this.#entries.get(keyOf(workspaceId, path));
		if (entry === undefined || !entry.state.dirty) return null;
		if (entry.saving || readAt < entry.touched || readAt < entry.seen) return entry.model;
		entry.seen = readAt;
		if (entry.notice?.kind !== 'conflict') {
			entry.notice = { kind: 'conflict', version: null };
			this.#refresh(entry, true);
		}
		return entry.model;
	}

	/**
	 * Saves the file's text over the version it was read at, or, with `expected`, over the version
	 * the person chose to overwrite (null to create a file that is gone). A file without edits and
	 * without a notice, or one already saving, is skipped.
	 */
	async save(
		source: Pick<WorkbenchWorkspaceFilesSource, 'save'>,
		workspaceId: string,
		path: string,
		overwrite?: { readonly expected: string | null },
	): Promise<SaveResult> {
		const entry = this.#entries.get(keyOf(workspaceId, path));
		if (entry === undefined || entry.saving) return 'skipped';
		const expected = overwrite === undefined ? (entry.base?.sha256 ?? null) : overwrite.expected;
		if (overwrite === undefined && (!entry.state.editable || !entry.state.dirty)) {
			return 'skipped';
		}
		const text = entry.model.getValue();
		if (!fitsSave(text)) {
			entry.notice = { kind: 'too-large' };
			this.#refresh(entry, true);
			return 'failed';
		}
		const sent = entry.model.getAlternativeVersionId();
		entry.saving = true;
		entry.touched = ++this.#clock;
		this.#refresh(entry, true);
		let outcome: Awaited<ReturnType<WorkbenchWorkspaceFilesSource['save']>>;
		try {
			outcome = await source.save(workspaceId, path, text, expected);
		} catch {
			outcome = null;
		}
		// A document released while it saved has nothing left to update.
		if (this.#entries.get(keyOf(workspaceId, path)) !== entry) return 'skipped';
		entry.saving = false;
		entry.touched = ++this.#clock;
		let result: SaveResult;
		if (outcome?.kind === 'written') {
			entry.base = outcome.version;
			entry.saved = sent;
			entry.notice = null;
			result = 'written';
		} else if (outcome?.kind === 'conflict') {
			entry.notice = { kind: 'conflict', version: outcome.version };
			result = 'conflict';
		} else {
			entry.notice = { kind: 'failed', reason: failureOf(outcome) };
			result = 'failed';
		}
		this.#refresh(entry, true);
		return result;
	}

	/**
	 * Throws the edits away and takes the file as it is on disk. A file that is gone keeps its text
	 * without edits until the next read shows that it is gone.
	 */
	async reload(
		source: Pick<WorkbenchWorkspaceFilesSource, 'read'>,
		workspaceId: string,
		path: string,
	): Promise<void> {
		const entry = this.#entries.get(keyOf(workspaceId, path));
		if (entry === undefined || entry.saving) return;
		const startedAt = ++this.#clock;
		entry.touched = startedAt;
		const whole = await readFileChunks(source, workspaceId, path, 'worktree', 'text');
		if (this.#entries.get(keyOf(workspaceId, path)) !== entry || entry.touched !== startedAt)
			return;
		entry.touched = ++this.#clock;
		if (whole.kind === 'text') {
			this.#apply(entry, whole.text, whole.version);
			return;
		}
		if (
			whole.kind === 'stopped' &&
			whole.outcome.kind === 'file' &&
			whole.outcome.content.kind === 'absent'
		) {
			entry.notice = null;
			entry.saved = entry.model.getAlternativeVersionId();
		} else {
			entry.notice = { kind: 'failed', reason: 'unavailable' };
		}
		this.#refresh(entry, true);
	}

	/** Clears a notice the person dismissed. */
	dismiss(workspaceId: string, path: string): void {
		const entry = this.#entries.get(keyOf(workspaceId, path));
		if (entry === undefined || entry.notice === null) return;
		entry.notice = null;
		this.#refresh(entry, true);
	}

	/** Releases every document no open tab shows, edits and all; `paths` maps each Workspace to those that stay. */
	retain(paths: ReadonlyMap<string, ReadonlySet<string>>): void {
		let changed = false;
		for (const [key, entry] of this.#entries) {
			const split = key.indexOf('\u0000');
			const kept = paths.get(key.slice(0, split))?.has(key.slice(split + 1)) ?? false;
			if (kept) continue;
			this.#release(entry);
			this.#entries.delete(key);
			changed = true;
		}
		if (changed) this.#notify();
	}

	/** Releases every document; the holder can still take new ones afterwards. */
	dispose(): void {
		for (const entry of this.#entries.values()) this.#release(entry);
		this.#entries.clear();
		this.#notify();
	}

	/** Takes text read from disk as the saved text, as one undoable edit. */
	#apply(entry: Entry<Model>, text: string, version: WorkspaceFileVersion | null): void {
		entry.applying = true;
		try {
			replaceText(entry.model, text);
		} finally {
			entry.applying = false;
		}
		entry.base = version;
		entry.saved = entry.model.getAlternativeVersionId();
		// The text now matches the disk, so nothing said about unsaved edits holds.
		entry.notice = null;
		this.#refresh(entry, true);
	}

	#release(entry: Entry<Model>): void {
		entry.listener.dispose();
		entry.model.dispose();
	}

	/** Recomputes a document's state; an edit notifies only when it changes whether it is dirty. */
	#refresh(entry: Entry<Model>, always: boolean): void {
		const next = snapshotOf(entry);
		if (!always && next.dirty === entry.state.dirty) return;
		entry.state = next;
		this.#notify();
	}

	#notify(): void {
		this.#version += 1;
		for (const listener of this.#listeners) listener();
	}
}
