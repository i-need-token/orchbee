/**
 * What `loadCodeEditor` resolves to: Monaco as `monaco-entry.ts` composes it, with the JSON
 * languages declared and the editor theme following the document's from the first load on, and the
 * documents and views that show files in it, which load with the editor rather than in the main
 * bundle. Kept apart from `monaco-entry.ts` so the modules it composes can take Monaco's types from
 * there without an import cycle.
 */
import { followCodeEditorTheme } from './followCodeEditorTheme';
import { editor, languages } from './monaco-entry';
import { registerJsonLanguages } from './registerJsonLanguages';

registerJsonLanguages(languages);
followCodeEditorTheme(window, { editor });

export { editor, languages };
export { CodeEditorModelRegistry } from './CodeEditorModelRegistry';
export { CodeEditorTextDocument } from './CodeEditorTextDocument';
export { CodeEditorViewStates } from './CodeEditorViewStates';
export { createCodeEditorFileDocuments } from './createCodeEditorFileDocuments';
export { ImagePreviewDocument } from './ImagePreviewDocument';
export { CodeEditorDiffView } from './components/code-editor-diff-view/CodeEditorDiffView';
export { CodeEditorMultiDiffView } from './components/code-editor-multi-diff-view/CodeEditorMultiDiffView';
export { CodeEditorTextView } from './components/code-editor-text-view/CodeEditorTextView';
export { ImagePreview } from './components/image-preview/ImagePreview';
export { imagePreviewMimeType } from './imagePreviewMimeType';
export { languageIdForPath } from './languageIdForPath';
export { unstageSelectedRanges } from './unstageSelectedRanges';
