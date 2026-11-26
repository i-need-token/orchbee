/**
 * The editor runtime the float window loads on first use: Monaco's core API, the editing features
 * a file and a diff need, and the syntax colorizers, which Monaco itself loads per language. It
 * carries no language service, so it starts no language Worker; its one Worker computes diffs and
 * word ranges, and starts from the bundled file so the page's script policy admits it.
 */
import * as monaco from 'monaco-editor/editor/editor.api';
import 'monaco-editor/features/bracketMatching/register';
import 'monaco-editor/features/caretOperations/register';
import 'monaco-editor/features/clipboard/register';
import 'monaco-editor/features/comment/register';
import 'monaco-editor/features/contextmenu/register';
import 'monaco-editor/features/cursorUndo/register';
import 'monaco-editor/features/diffEditor/register';
import 'monaco-editor/features/find/register';
import 'monaco-editor/features/folding/register';
import 'monaco-editor/features/gotoLine/register';
import 'monaco-editor/features/indentation/register';
import 'monaco-editor/features/lineSelection/register';
import 'monaco-editor/features/linesOperations/register';
import 'monaco-editor/features/multicursor/register';
import 'monaco-editor/features/readOnlyMessage/register';
import 'monaco-editor/features/smartSelect/register';
import 'monaco-editor/features/unicodeHighlighter/register';
import 'monaco-editor/features/wordHighlighter/register';
import 'monaco-editor/features/wordOperations/register';
import 'monaco-editor/languages/definitions/register.all';

import { registerJsonColorizer } from './registerJsonColorizer';
import { followEditorTheme } from './monaco-theme';

export type Monaco = typeof monaco;

let started = false;

/** Starts the runtime once per page: the Worker route, the JSON colorizer and the theme. */
export function startMonaco(targetWindow: Window): Monaco {
	if (started) return monaco;
	started = true;
	globalThis.MonacoEnvironment = {
		getWorker: () =>
			new Worker(new URL('monaco-editor/editor/editor.worker', import.meta.url), {
				type: 'module',
			}),
	};
	registerJsonColorizer(monaco);
	followEditorTheme(targetWindow, monaco);
	return monaco;
}
