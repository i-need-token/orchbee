import {
	filePathKey,
	isFilePathWithin,
	type FileChange,
	type FileGrantSource,
	type FilePathRules,
	type FileResource,
	type FileRootEvent,
	type FileWorkspaceSession,
} from '../file-session';
import { CodeEditorTextDocument, type CodeEditorTextContent } from './CodeEditorTextDocument';
import { CodeEditorViewStates } from './CodeEditorViewStates';
import { ImagePreviewDocument } from './ImagePreviewDocument';
import type { ImageViewerViewState } from './ImageViewer';
import { imagePreviewMimeType } from './imagePreviewMimeType';

const MEBIBYTE = 1024 * 1024;

/**
 * VS Code's `workbench.editorLargeFileConfirmation`: a file on this computer opens up to 1024 MB
 * before it asks, one on another machine up to 10 MB, as in a VS Code remote window.
 */
const LOCAL_LIMIT = 1024 * MEBIBYTE;
const REMOTE_LIMIT = 10 * MEBIBYTE;

/** The image preview reads the whole file, as VS Code's does; the backend keeps its own ceiling. */
const IMAGE_LIMIT = Number.MAX_SAFE_INTEGER;

/** A hold on the Workspace's file session, which stays open while any hold does. */
export interface CodeEditorFileSessionLease {
	readonly session: FileWorkspaceSession;
	/** Ends this hold; releasing it again does nothing. */
	release(): void;
}

/** How the documents make and refresh the models of the text they read. */
interface CodeEditorFileModels<Model> {
	/** Creates the model for a file's text, in the language its path selects. */
	create(content: CodeEditorTextContent, path: string): Model;
	/** Takes a reload's text into the model. */
	update(model: Model, content: CodeEditorTextContent): void;
}

interface CodeEditorFileDocumentsOptions<Model> {
	/** Holds the Workspace's file session; the documents hold it while any is open. */
	readonly lease: () => CodeEditorFileSessionLease;
	readonly models: CodeEditorFileModels<Model>;
}

/** How a file opens. */
interface CodeEditorFileOpenOptions {
	/** The gesture that opened it, which the backend admits it by. */
	readonly source: FileGrantSource;
	/**
	 * Opens it in the text editor whatever its name, as Open in Text Editor does; otherwise an image
	 * opens in the image preview.
	 */
	readonly text: boolean;
}

interface CodeEditorFileHoldBase {
	/** The key the file's view state is kept under. */
	readonly viewStateKey: string;
	/** Ends this hold; releasing it again does nothing. */
	release(): void;
}

/** One holder's share of a file's document, and where its editor was left. */
export type CodeEditorFileHold<
	Model extends { dispose(): void },
	TextViewState,
> = CodeEditorFileHoldBase &
	(
		| {
				readonly document: ImagePreviewDocument;
				readonly kind: 'image';
				readonly viewStates: CodeEditorViewStates<ImageViewerViewState>;
		  }
		| {
				readonly document: CodeEditorTextDocument<Model>;
				readonly kind: 'text';
				readonly viewStates: CodeEditorViewStates<TextViewState>;
		  }
	);

type FileDocument<Model extends { dispose(): void }> =
	| { readonly document: ImagePreviewDocument; readonly kind: 'image' }
	| { readonly document: CodeEditorTextDocument<Model>; readonly kind: 'text' };

/** A document and the holds on it. */
type DocumentEntry<Model extends { dispose(): void }> = FileDocument<Model> & {
	holders: number;
	readonly resource: FileResource;
};

/** A file some document shows, granted to the session while any does. */
interface OpenedFile {
	readonly grant: AbortController;
	/** Settles once the grant does, given or not; a read waits for it, so it may pass the root. */
	readonly granted: Promise<void>;
	/** Whether the backend holds the grant, which closing the file gives back. */
	held: boolean;
	readonly path: string;
	shownIn: number;
}

/**
 * One Workspace's files as the overlay's tabs show them: each file's text or image document, shared
 * by every holder that shows it in the same editor, and where each file's editors were left. A file
 * is granted while a document shows it, so its changes reach the documents, which read it again on
 * a change to it, to a folder above it that is added or deleted, a rescan at or above it, and the
 * snapshot that ends a gap in the stream. The session is held while any document is open.
 */
export class CodeEditorFileDocuments<Model extends { dispose(): void }, TextViewState> {
	public readonly imageViewStates = new CodeEditorViewStates<ImageViewerViewState>();
	public readonly textViewStates = new CodeEditorViewStates<TextViewState>();
	private disposed = false;
	private readonly documents = new Map<string, DocumentEntry<Model>>();
	/** By the file's key. */
	private readonly files = new Map<string, OpenedFile>();
	private lease: CodeEditorFileSessionLease | null = null;
	private readonly options: CodeEditorFileDocumentsOptions<Model>;
	private rules: FilePathRules | null = null;
	/** Records were lost since the last snapshot, so the next one reads every file again. */
	private stale = false;
	private unfollow: (() => void) | null = null;

	public constructor(options: CodeEditorFileDocumentsOptions<Model>) {
		this.options = options;
	}

	/** Holds a file's document, opening it with the first hold; disposed documents open nothing. */
	public open(
		resource: FileResource,
		options: CodeEditorFileOpenOptions,
	): CodeEditorFileHold<Model, TextViewState> {
		if (this.disposed) throw new Error('The file documents were disposed of.');
		const kind =
			options.text || imagePreviewMimeType(resource.path) === undefined ? 'text' : 'image';
		const id = `${kind}:${resource.key}`;
		let entry = this.documents.get(id);
		if (entry === undefined) {
			entry = this.create(kind, resource, options.source);
			this.documents.set(id, entry);
		}
		entry.holders += 1;
		const held = entry;
		let released = false;
		const release = (): void => {
			if (released) return;
			released = true;
			held.holders -= 1;
			if (held.holders > 0 || this.documents.get(id) !== held) return;
			this.documents.delete(id);
			this.close(held);
		};
		const viewStateKey = resource.key;
		return held.kind === 'text'
			? {
					document: held.document,
					kind: 'text',
					release,
					viewStateKey,
					viewStates: this.textViewStates,
				}
			: {
					document: held.document,
					kind: 'image',
					release,
					viewStateKey,
					viewStates: this.imageViewStates,
				};
	}

	/** Closes every document, gives back every grant and lets go of the session. */
	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		const entries = [...this.documents.values()];
		this.documents.clear();
		for (const entry of entries) this.close(entry);
	}

	private create(
		kind: 'image' | 'text',
		resource: FileResource,
		source: FileGrantSource,
	): DocumentEntry<Model> {
		const lease = this.hold();
		const file = this.openFile(lease.session, resource, source);
		const { content } = lease.session;
		const { path } = resource;
		if (kind === 'image') {
			const document = new ImagePreviewDocument({
				path,
				read: (signal) =>
					file.granted.then(() =>
						content.read({ limit: IMAGE_LIMIT, mode: 'bytes', path }, signal),
					),
			});
			return { document, holders: 0, kind, resource };
		}
		const { models } = this.options;
		const document = new CodeEditorTextDocument<Model>({
			createModel: (text) => models.create(text, path),
			limit: lease.session.remote ? REMOTE_LIMIT : LOCAL_LIMIT,
			read: ({ forceText, limit }, signal) =>
				file.granted.then(() => content.read({ forceText, limit, mode: 'text', path }, signal)),
			updateModel: (model, text) => models.update(model, text),
		});
		return { document, holders: 0, kind, resource };
	}

	/** Grants a file as it opens in its first document, and counts the documents that show it. */
	private openFile(
		session: FileWorkspaceSession,
		resource: FileResource,
		source: FileGrantSource,
	): OpenedFile {
		const opened = this.files.get(resource.key);
		if (opened !== undefined) {
			opened.shownIn += 1;
			return opened;
		}
		const grant = new AbortController();
		const file: OpenedFile = {
			grant,
			granted: session.grants.grant(resource.path, source, grant.signal).then(
				(result) => {
					if (result.kind !== 'ok') return;
					// Closed between the grant landing and this answer, too late for the abort.
					if (grant.signal.aborted) session.grants.ungrant([resource.path]);
					else file.held = true;
				},
				() => undefined,
			),
			held: false,
			path: resource.path,
			shownIn: 1,
		};
		this.files.set(resource.key, file);
		return file;
	}

	private close(entry: DocumentEntry<Model>): void {
		entry.document.dispose();
		const file = this.files.get(entry.resource.key);
		if (file === undefined) return;
		file.shownIn -= 1;
		if (file.shownIn > 0) return;
		this.files.delete(entry.resource.key);
		// A grant still being asked for is given back once it lands.
		if (file.held) this.lease?.session.grants.ungrant([file.path]);
		else file.grant.abort();
		if (this.files.size === 0) this.letGo();
	}

	/** The session, held from the first open file on and followed for changes to the files. */
	private hold(): CodeEditorFileSessionLease {
		if (this.lease !== null) return this.lease;
		const lease = this.options.lease();
		this.lease = lease;
		this.unfollow = lease.session.subscribe((event) => this.follow(event));
		return lease;
	}

	private letGo(): void {
		this.unfollow?.();
		this.unfollow = null;
		this.lease?.release();
		this.lease = null;
		this.rules = null;
		this.stale = false;
	}

	private follow(event: FileRootEvent): void {
		switch (event.kind) {
			case 'snapshot':
				this.rules = event.rules;
				if (this.stale) this.reloadWhere(() => true);
				this.stale = false;
				return;
			case 'gap':
				this.stale = true;
				return;
			case 'update': {
				const { rules } = this;
				if (rules === null) return;
				this.reloadWhere(
					(path) =>
						event.rescan.some((under) => isFilePathWithin(path, under, rules)) ||
						event.changes.some((change) => reaches(change, path, rules)),
				);
			}
		}
	}

	private reloadWhere(changed: (path: string) => boolean): void {
		for (const entry of this.documents.values()) {
			if (changed(entry.resource.path)) entry.document.reload();
		}
	}
}

/**
 * Whether a change may have changed the file at `path`: a change to the file itself, or a folder
 * above it that was added or deleted, which brings the file or takes it away.
 */
function reaches(change: FileChange, path: string, rules: FilePathRules): boolean {
	if (!change.directory) return filePathKey(change.path, rules) === filePathKey(path, rules);
	return change.type !== 'changed' && isFilePathWithin(path, change.path, rules);
}
