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
export { CodeEditorTextView } from './components/code-editor-text-view/CodeEditorTextView';
export { languageIdForPath } from './languageIdForPath';
