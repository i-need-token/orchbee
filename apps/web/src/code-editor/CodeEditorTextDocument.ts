import type { FileContent, FileError, FileResult } from '../file-session';

/** The ceiling Open Anyway reads up to, as VS Code's `Number.MAX_VALUE` does. */
const LIFTED_LIMIT = Number.MAX_SAFE_INTEGER;

/** What a text read decoded, which the model holds and the info row describes. */
interface CodeEditorTextContent {
	readonly bom: boolean;
	readonly encoding: string;
	readonly lineEnding: 'crlf' | 'lf' | 'none';
	readonly text: string;
}

/** A shown file as the text editor sees it, by VS Code's placeholders for content it refuses. */
export type CodeEditorTextDocumentState<Model> =
	| { readonly kind: 'binary' }
	| { readonly error: FileError; readonly kind: 'error' }
	| { readonly kind: 'loading' }
	| {
			readonly content: Omit<CodeEditorTextContent, 'text'>;
			readonly kind: 'text';
			readonly model: Model;
	  }
	/** `lifted` once Open Anyway read past the Client's ceiling and the backend still refused. */
	| { readonly kind: 'too-large'; readonly lifted: boolean };

interface CodeEditorTextDocumentOptions<Model> {
	/** Creates the model from the text a read decoded; the document disposes of it. */
	readonly createModel: (content: CodeEditorTextContent) => Model;
	/** The Client's ceiling in bytes before VS Code's large-file confirmation. */
	readonly limit: number;
	/** Reads the file as text, as `FileContentPort.read` does with the document's path. */
	readonly read: (
		options: { readonly forceText: boolean; readonly limit: number },
		signal: AbortSignal,
	) => Promise<FileResult<FileContent>>;
}

/** A read that rejected, answered with bytes, or whose model could not be created. */
const FAILED: FileError = { code: 'failed' };

/**
 * One file's text for the editors that show it, loaded when created. Content the text editor
 * refuses, binary or larger than the Client's ceiling, waits for Open Anyway; a failed read waits
 * for Try Again. Disposing of the document abandons its read and disposes of its model.
 */
export class CodeEditorTextDocument<Model extends { dispose(): void }> {
	private controller: AbortController | null = null;
	private disposed = false;
	private forceText = false;
	private limit: number;
	private readonly listeners = new Set<() => void>();
	private readonly options: CodeEditorTextDocumentOptions<Model>;
	private state: CodeEditorTextDocumentState<Model> = { kind: 'loading' };

	public constructor(options: CodeEditorTextDocumentOptions<Model>) {
		this.options = options;
		this.limit = options.limit;
		this.load();
	}

	public readonly getState = (): CodeEditorTextDocumentState<Model> => this.state;

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	/** Open Anyway: decodes binary content as text, or reads past the Client's ceiling. */
	public openAnyway(): void {
		if (this.state.kind === 'binary') {
			this.forceText = true;
		} else if (this.state.kind === 'too-large' && !this.state.lifted) {
			this.limit = LIFTED_LIMIT;
		} else {
			return;
		}
		this.load();
	}

	/** Try Again: reads the file again after a failed read. */
	public retry(): void {
		if (this.state.kind === 'error') this.load();
	}

	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.controller?.abort();
		this.controller = null;
		if (this.state.kind === 'text') this.state.model.dispose();
		this.listeners.clear();
	}

	private load(): void {
		this.controller?.abort();
		const controller = new AbortController();
		this.controller = controller;
		this.update({ kind: 'loading' });
		const settle = (next: () => CodeEditorTextDocumentState<Model>): void => {
			// A disposed document, or one that started another read, ignores this answer.
			if (this.disposed || this.controller !== controller) return;
			let state: CodeEditorTextDocumentState<Model>;
			try {
				state = next();
			} catch {
				state = { error: FAILED, kind: 'error' };
			}
			this.controller = null;
			this.update(state);
		};
		void this.options
			.read({ forceText: this.forceText, limit: this.limit }, controller.signal)
			.then(
				(result) => settle(() => this.settled(result)),
				() => settle(() => ({ error: FAILED, kind: 'error' })),
			);
	}

	private settled(result: FileResult<FileContent>): CodeEditorTextDocumentState<Model> {
		if (result.kind === 'error') {
			return result.error.code === 'too-large'
				? { kind: 'too-large', lifted: this.limit === LIFTED_LIMIT }
				: { error: result.error, kind: 'error' };
		}
		const content = result.value;
		if (content.kind === 'binary') return { kind: 'binary' };
		// A text read never answers with bytes.
		if (content.kind === 'bytes') return { error: FAILED, kind: 'error' };
		const { bom, encoding, lineEnding, text } = content;
		const model = this.options.createModel({ bom, encoding, lineEnding, text });
		return { content: { bom, encoding, lineEnding }, kind: 'text', model };
	}

	private update(state: CodeEditorTextDocumentState<Model>): void {
		this.state = state;
		for (const listener of this.listeners) listener();
	}
}
