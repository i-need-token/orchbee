import type { CodeEditorOverlayDocuments, CodeEditorOverlayHold } from '../../code-editor';
import type { FileSessionRegistry } from '../../file-session';
import type { FileOverlayStore } from './FileOverlayRegistry';

/** Holds the Workspace's file session for the documents. */
type FileTabSessionLease = () => ReturnType<FileSessionRegistry['hold']>;

/** What the content area shows: the active tab's file and the document it holds. */
export interface FileTabShown {
	readonly hold: CodeEditorOverlayHold;
	/** The file's path, as the Workspace's file session names it. */
	readonly path: string;
	readonly tabId: string;
}

/**
 * The content behind one Workspace's file tabs. A tab holds its file's document from the first time
 * it is active, as VS Code resolves an editor's model when the editor first shows, until it leaves
 * the row, closed or replaced as the preview, so switching back to it shows the document as it was.
 * Documents come from the code editor, which loads with the content area, so no tab holds one until
 * it is attached; from then on the tabs are followed whether the overlay shows or not.
 */
export class FileTabContents {
	/** Tabs that show their file in the text editor whatever its name, by Open in Text Editor. */
	private readonly asText = new Set<string>();
	private disposed = false;
	private documents: CodeEditorOverlayDocuments | null = null;
	private readonly holds = new Map<string, CodeEditorOverlayHold>();
	private readonly lease: FileTabSessionLease;
	private readonly listeners = new Set<() => void>();
	/** Restored tabs not yet shown, whose files the backend admits as restored ones. */
	private readonly restored: Set<string>;
	private shown: FileTabShown | null = null;
	private unfollow: (() => void) | null = null;
	public readonly store: FileOverlayStore;

	public constructor(store: FileOverlayStore, lease: FileTabSessionLease) {
		this.lease = lease;
		this.store = store;
		this.restored = new Set(store.state.getInitialState().tabs.tabs.map((tab) => tab.id));
	}

	/** What the content area shows now; null without tabs, or until the documents are attached. */
	public readonly getShown = (): FileTabShown | null => this.shown;

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	/** Gives the tabs their documents once the code editor has loaded; later calls change nothing. */
	public attach(create: (lease: FileTabSessionLease) => CodeEditorOverlayDocuments): void {
		if (this.documents !== null || this.disposed) return;
		this.documents = create(this.lease);
		this.unfollow = this.store.state.subscribe(() => this.follow());
		this.follow();
	}

	/** Open in Text Editor: the tab shows its file in the text editor while it stays open. */
	public openAsText(tabId: string): void {
		if (this.disposed || this.asText.has(tabId)) return;
		this.asText.add(tabId);
		this.holds.get(tabId)?.release();
		this.holds.delete(tabId);
		this.follow();
	}

	/** Lets go of every document, as the Workspace's tabs go with it. */
	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.unfollow?.();
		this.unfollow = null;
		this.holds.clear();
		this.documents?.dispose();
		this.documents = null;
		this.shown = null;
		this.notify();
		this.listeners.clear();
	}

	/** Lets go of the documents of tabs that left the row, and holds the active tab's. */
	private follow(): void {
		const { documents } = this;
		if (documents === null) return;
		const { tabs } = this.store.state.getState();
		const open = new Set(tabs.tabs.map((tab) => tab.id));
		for (const [tabId, hold] of this.holds) {
			if (open.has(tabId)) continue;
			this.holds.delete(tabId);
			hold.release();
		}
		for (const tabIds of [this.asText, this.restored]) {
			for (const tabId of tabIds) if (!open.has(tabId)) tabIds.delete(tabId);
		}
		const active = tabs.tabs.find((tab) => tab.id === tabs.activeId);
		let shown: FileTabShown | null = null;
		if (active !== undefined) {
			let hold = this.holds.get(active.id);
			if (hold === undefined) {
				hold = documents.open(active.input.resource, {
					source: this.restored.delete(active.id) ? 'restored' : 'tree',
					text: this.asText.has(active.id),
				});
				this.holds.set(active.id, hold);
			}
			shown =
				this.shown?.hold === hold
					? this.shown
					: { hold, path: active.input.resource.path, tabId: active.id };
		}
		if (shown === this.shown) return;
		this.shown = shown;
		this.notify();
	}

	private notify(): void {
		for (const listener of this.listeners) listener();
	}
}
