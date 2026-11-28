import {
	CodeEditorFileDocuments,
	type CodeEditorFileHold,
	type CodeEditorFileSessionLease,
} from './CodeEditorFileDocuments';
import type { CodeEditorTextContent } from './CodeEditorTextDocument';
import { codeEditorTextEdit } from './codeEditorTextEdit';
import { languageIdForPath } from './languageIdForPath';
import type { editor, languages } from './monaco-entry';

/** Monaco, from what `loadCodeEditor` resolves to. */
interface CodeEditorFileMonaco {
	readonly editor: Pick<typeof editor, 'createModel' | 'EndOfLineSequence'>;
	readonly languages: Pick<typeof languages, 'getLanguages'>;
}

/** The overlay's documents, whose text shows in Monaco's models. */
export type CodeEditorOverlayDocuments = CodeEditorFileDocuments<
	editor.ITextModel,
	editor.ICodeEditorViewState
>;

/** A tab's hold on one of the overlay's documents. */
export type CodeEditorOverlayHold = CodeEditorFileHold<
	editor.ITextModel,
	editor.ICodeEditorViewState
>;

/** Every line break Monaco splits text at. */
const LINE_BREAK = /\r\n|\r|\n/u;

/**
 * Takes a reload's text into a model as VS Code's reload does: the line ending first, then one edit
 * of the lines that changed, the two together one step of the undo stack.
 */
function updateTextModel(
	monaco: CodeEditorFileMonaco,
	model: editor.ITextModel,
	content: CodeEditorTextContent,
): void {
	const { EndOfLineSequence } = monaco.editor;
	let lineEnding: editor.EndOfLineSequence | null = null;
	if (content.lineEnding === 'crlf') lineEnding = EndOfLineSequence.CRLF;
	else if (content.lineEnding === 'lf') lineEnding = EndOfLineSequence.LF;
	// Text without a line break keeps the model's line ending, as it would keep VS Code's.
	if (lineEnding === model.getEndOfLineSequence()) lineEnding = null;
	const edit = codeEditorTextEdit(model, content.text.split(LINE_BREAK));
	if (lineEnding === null && edit === null) return;
	model.pushStackElement();
	if (lineEnding !== null) model.pushEOL(lineEnding);
	if (edit !== null) model.pushEditOperations([], [edit], () => null);
	model.pushStackElement();
}

/** One Workspace's documents for the overlay's tabs, holding its session through `lease`. */
export function createCodeEditorFileDocuments(
	monaco: CodeEditorFileMonaco,
	lease: () => CodeEditorFileSessionLease,
): CodeEditorOverlayDocuments {
	return new CodeEditorFileDocuments({
		lease,
		models: {
			create: (content, path) =>
				monaco.editor.createModel(
					content.text,
					languageIdForPath(monaco.languages.getLanguages(), path),
				),
			update: (model, content) => updateTextModel(monaco, model, content),
		},
	});
}
