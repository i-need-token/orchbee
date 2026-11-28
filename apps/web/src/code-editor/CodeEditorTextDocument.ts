import type { FileContent, FileError, FileResult } from '../file-session';

/** The ceiling Open Anyway reads up to, as VS Code's `Number.MAX_VALUE` does. */
const LIFTED_LIMIT = Number.MAX_SAFE_INTEGER;

/** What a text read decoded, which the model holds and the info row describes. */
export interface CodeEditorTextContent {
	readonly bom: boolean;
	readonly encoding: string;
	readonly lineEnding: 'crlf' | 'lf' | 'none';
	readonly text: string;
}

interface CodeEditorTextState<Model> {
	readonly content: Omit<CodeEditorTextContent, 'text'>;
	readonly kind: 'text';
	readonly model: Model;
}

/** A shown file as the text editor sees it, by VS Code's placeholders for content it refuses. */
export type CodeEditorTextDocumentState<Model> =
	| { readonly kind: 'binary' }
	| { readonly error: FileError; readonly kind: 'error' }
	| { readonly kind: 'loading' }
	| CodeEditorTextState<Model>
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
	/** Takes the text a reload decoded into the model the document shows. */
	readonly updateModel: (model: Model, content: CodeEditorTextContent) => void;
}

/** A read that rejected, answered with bytes, or whose model could not be created. */
const FAILED: FileError = { code: 'failed' };
const FAILED_STATE = { error: FAILED, kind: 'error' } as const;

/**
 * One file's text for the editors that show it, loaded when created. Content the text editor
 * refuses, binary or larger than the Client's ceiling, waits for Open Anyway; a failed read waits
 * for Try Again. A reload after the file changed keeps what the document shows until its read
 * lands. Disposing of the document abandons its read and disposes of its model.
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

	/**
	 * Reads the file again after it changed on disk. Shown text stays text, as a resolved VS Code
	 * model does: it reads past the binary check and the Client's ceiling, and its new text goes
	 * into the same model, so the editors keep their view. A failed read keeps the text, as VS Code
	 * keeps a model whose file is missing. A placeholder stays until the read lands, and a load
	 * still running starts again, so it reads what changed.
	 */
	public reload(): void {
		if (this.disposed) return;
		if (this.state.kind === 'loading') this.load();
		else this.read();
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
		this.update({ kind: 'loading' });
		this.read();
	}

	/** Reads the file, replacing any read still running; the answer settles the document. */
	private read(): void {
		this.controller?.abort();
		const controller = new AbortController();
		this.controller = controller;
		const shown = this.state.kind === 'text' ? this.state : null;
		// A reload that fails keeps the text, and nothing else changes the state.
		const failed = shown === null ? FAILED_STATE : null;
		const settle = (next: () => CodeEditorTextDocumentState<Model> | null): void => {
			// A disposed document, or one that started another read, ignores this answer.
			if (this.disposed || this.controller !== controller) return;
			let state: CodeEditorTextDocumentState<Model> | null;
			try {
				state = next();
			} catch {
				state = failed;
			}
			this.controller = null;
			if (state !== null) this.update(state);
		};
		const options =
			shown === null
				? { forceText: this.forceText, limit: this.limit }
				: { forceText: true, limit: LIFTED_LIMIT };
		void this.options.read(options, controller.signal).then(
			(result) =>
				settle(() => (shown === null ? this.settled(result) : this.reloaded(shown, result))),
			() => settle(() => failed),
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

	private reloaded(
		shown: CodeEditorTextState<Model>,
		result: FileResult<FileContent>,
	): CodeEditorTextDocumentState<Model> | null {
		if (result.kind === 'error' || result.value.kind !== 'text') return null;
		const { bom, encoding, lineEnding, text } = result.value;
		this.options.updateModel(shown.model, { bom, encoding, lineEnding, text });
		return { content: { bom, encoding, lineEnding }, kind: 'text', model: shown.model };
	}

	private update(state: CodeEditorTextDocumentState<Model>): void {
		this.state = state;
		for (const listener of this.listeners) listener();
	}
}
