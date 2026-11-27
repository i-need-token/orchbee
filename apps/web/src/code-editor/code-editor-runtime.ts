/**
 * What `loadCodeEditor` resolves to: Monaco as `monaco-entry.ts` composes it, with the editor
 * theme following the document's from the first load on. Kept apart from `monaco-entry.ts` so the
 * modules it composes can take Monaco's types from there without an import cycle.
 */
import { followCodeEditorTheme } from './followCodeEditorTheme';
import { editor, languages } from './monaco-entry';

followCodeEditorTheme(window, { editor });

export { editor, languages };
