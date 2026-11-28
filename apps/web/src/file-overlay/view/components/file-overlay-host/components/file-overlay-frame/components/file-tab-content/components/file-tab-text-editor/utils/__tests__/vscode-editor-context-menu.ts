/**
 * VS Code's editor context menu, `MenuId.EditorContext` with the built-in extensions'
 * `editor/context` entries, transcribed from Visual Studio Code (Code - OSS) at commit
 * ef5ca45a6e545e4767e0117905ee9d6120b86e11.
 *
 * The items stand in VS Code's menu order. An item Orchbee offers keeps VS Code's `when` over the
 * context keys below, which for an editor action includes its precondition; every other item names
 * the reason it is left out.
 *
 * MIT License
 *
 * Copyright (c) 2015 - present Microsoft Corporation
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
 * associated documentation files (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge, publish, distribute,
 * sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all copies or
 * substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
 * NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
 * DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
 * OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

/** The context keys the offered items read, by VS Code's names, about the editor the menu is for. */
type VSCodeEditorContextKey = 'editorReadonly' | 'editorSimpleInput' | 'editorTextFocus';

export type VSCodeEditorCondition =
	| { readonly and: readonly VSCodeEditorCondition[] }
	| { readonly key: VSCodeEditorContextKey }
	| { readonly not: VSCodeEditorCondition };

export interface VSCodeEditorMenuItem {
	readonly command: string;
	readonly group: string;
	readonly orchbee:
		| { readonly kind: 'excluded'; readonly reason: string }
		| { readonly kind: 'offered' }
		| { readonly kind: 'offered'; readonly label: string; readonly why: string };
	readonly order: number;
	readonly title: string;
	readonly when?: VSCodeEditorCondition;
}

const WRITABLE: VSCodeEditorCondition = { not: { key: 'editorReadonly' } };

export const VSCODE_EDITOR_CONTEXT: readonly VSCodeEditorMenuItem[] = [
	{
		command: 'npm.runSelectedScript',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Tasks' },
		order: 1,
		title: 'Run Script',
	},
	{
		command: 'editor.action.revealDefinition',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.1,
		title: 'Go to Definition',
	},
	{
		command: 'editor.action.revealDeclaration',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.3,
		title: 'Go to Declaration',
	},
	{
		command: 'editor.action.goToTypeDefinition',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.4,
		title: 'Go to Type Definition',
	},
	{
		command: 'typescript.goToSourceDefinition',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.41,
		title: 'Go to Source Definition',
	},
	{
		command: 'editor.action.goToImplementation',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.45,
		title: 'Go to Implementations',
	},
	{
		command: 'editor.action.goToReferences',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.45,
		title: 'Go to References',
	},
	{
		command: 'submenu:EditorContextPeek',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 100,
		title: 'Peek',
	},
	{
		command: 'references-view.findReferences',
		group: '0_navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1,
		title: 'Find All References',
	},
	{
		command: 'references-view.findImplementations',
		group: '0_navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 2,
		title: 'Find All Implementations',
	},
	{
		command: 'references-view.showCallHierarchy',
		group: '0_navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 3,
		title: 'Show Call Hierarchy',
	},
	{
		command: 'references-view.showTypeHierarchy',
		group: '0_navigation',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 4,
		title: 'Show Type Hierarchy',
	},
	{
		command: 'workbench.action.chat.attachSelection',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 1,
		title: 'Add Selection to Chat',
	},
	{
		command: 'workbench.action.chat.attachFile',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 2,
		title: 'Add File to Chat',
	},
	{
		command: 'inlineChat.askInChat',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 3,
		title: 'Ask in Chat',
	},
	{
		command: 'inlineChat.start',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 3,
		title: 'Open Inline Chat',
	},
	{
		command: 'chat.internal.explain',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 4,
		title: 'Explain',
	},
	{
		command: 'github.copilot.chat.fix',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 4,
		title: 'Fix',
	},
	{
		command: 'github.copilot.chat.explain',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 5,
		title: 'Explain',
	},
	{
		command: 'chat.internal.fix',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 5,
		title: 'Fix',
	},
	{
		command: 'submenu:ChatTextEditorMenu',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 5,
		title: 'Generate Code',
	},
	{
		command: 'chat.internal.review',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 6,
		title: 'Code Review',
	},
	{
		command: 'github.copilot.chat.review',
		group: '1_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 6,
		title: 'Review',
	},
	{
		command: 'editor.action.rename',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.1,
		title: 'Rename Symbol',
	},
	{
		command: 'editor.action.changeAll',
		group: '1_modification',
		orchbee: { kind: 'offered' },
		order: 1.2,
		title: 'Change All Occurrences',
		when: { and: [WRITABLE, { key: 'editorTextFocus' }] },
	},
	{
		command: 'editor.action.formatDocument',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.3,
		title: 'Format Document',
	},
	{
		command: 'editor.action.formatDocument.multiple',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.3,
		title: 'Format Document With...',
	},
	{
		command: 'notebook.formatCell',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Notebooks' },
		order: 1.301,
		title: 'Format Cell',
	},
	{
		command: 'editor.action.formatSelection',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.31,
		title: 'Format Selection',
	},
	{
		command: 'editor.action.formatSelection.multiple',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 1.31,
		title: 'Format Selection With...',
	},
	{
		command: 'editor.action.refactor',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 2,
		title: 'Refactor...',
	},
	{
		command: 'editor.action.sourceAction',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 2.1,
		title: 'Source Action...',
	},
	{
		command: 'workbench.action.editorDictation.start',
		group: '1_modification',
		orchbee: { kind: 'excluded', reason: 'Speech, which only an extension provides' },
		order: 6,
		title: 'Start Dictation in Editor',
	},
	{
		command: 'submenu:EditorContextShare',
		group: '11_share',
		orchbee: { kind: 'excluded', reason: 'Share' },
		order: -1,
		title: 'Share',
	},
	{
		command: 'git.stageSelectedRanges',
		group: '2_git',
		orchbee: { kind: 'excluded', reason: 'Diff editors, which the overlay’s text editor is not' },
		order: 1,
		title: 'Stage Selected Ranges',
	},
	{
		command: 'git.unstageSelectedRanges',
		group: '2_git',
		orchbee: { kind: 'excluded', reason: 'Diff editors, which the overlay’s text editor is not' },
		order: 2,
		title: 'Unstage Selected Ranges',
	},
	{
		command: 'git.revertSelectedRanges',
		group: '2_git',
		orchbee: { kind: 'excluded', reason: 'Diff editors, which the overlay’s text editor is not' },
		order: 3,
		title: 'Revert Selected Ranges',
	},
	{
		command: 'submenu:NotebookCellTitle',
		group: '2_insert',
		orchbee: { kind: 'excluded', reason: 'Notebooks' },
		order: 0,
		title: 'Notebook Cell',
	},
	{
		command: 'editor.action.clipboardCutAction',
		group: '9_cutcopypaste',
		orchbee: { kind: 'offered' },
		order: 1,
		title: 'Cut',
		when: WRITABLE,
	},
	{
		command: 'editor.action.clipboardCopyAction',
		group: '9_cutcopypaste',
		orchbee: { kind: 'offered' },
		order: 2,
		title: 'Copy',
	},
	{
		command: 'submenu:EditorContextCopy',
		group: '9_cutcopypaste',
		orchbee: { kind: 'excluded', reason: 'Copy As, which nothing in VS Code fills' },
		order: 3,
		title: 'Copy As',
	},
	{
		command: 'editor.action.clipboardPasteAction',
		group: '9_cutcopypaste',
		orchbee: { kind: 'offered' },
		order: 4,
		title: 'Paste',
		when: WRITABLE,
	},
	{
		command: 'github.copilot.chat.copilotCLI.addFileReference',
		group: 'copilot',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 0,
		title: 'Add File to Copilot CLI',
	},
	{
		command: 'github.copilot.chat.copilotCLI.addSelection',
		group: 'copilot',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 0,
		title: 'Add Selection to Copilot CLI',
	},
	{
		command: 'editor.debug.action.selectionToRepl',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 0,
		title: 'Evaluate in Debug Console',
	},
	{
		command: 'editor.debug.action.toggleInlineBreakpoint',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 1,
		title: 'Add Inline Breakpoint',
	},
	{
		command: 'editor.debug.action.selectionToWatch',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 1,
		title: 'Add to Watch',
	},
	{
		command: 'editor.debug.action.stepIntoTargets',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 1.5,
		title: 'Step Into Target',
	},
	{
		command: 'editor.debug.action.runToCursor',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 2,
		title: 'Run to Cursor',
	},
	{
		command: 'debug.jumpToCursor',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 3,
		title: 'Jump to Cursor',
	},
	{
		command: 'debug.action.openDisassemblyView',
		group: 'debug',
		orchbee: { kind: 'excluded', reason: 'Debugging' },
		order: 5,
		title: 'Open Disassembly View',
	},
	{
		command: 'testing.runAtCursor',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 0,
		title: 'Run Test at Cursor',
	},
	{
		command: 'testing.debugAtCursor',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 1,
		title: 'Debug Test at Cursor',
	},
	{
		command: 'testing.runCurrentFile',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 2,
		title: 'Run Tests in Current File',
	},
	{
		command: 'testing.debugCurrentFile',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 3,
		title: 'Debug Tests in Current File',
	},
	{
		command: 'testing.goToRelatedCode',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 4,
		title: 'Go to Related Code',
	},
	{
		command: 'testing.goToRelatedTest',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 4,
		title: 'Go to Related Test',
	},
	{
		command: 'testing.peekRelatedCode',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 5,
		title: 'Peek Related Code',
	},
	{
		command: 'testing.peekRelatedTest',
		group: 'testing',
		orchbee: { kind: 'excluded', reason: 'Testing, which only an extension provides' },
		order: 5,
		title: 'Peek Related Test',
	},
	{
		command: 'workbench.action.showCommands',
		group: 'z_commands',
		orchbee: {
			kind: 'offered',
			label: 'Search Actions…',
			why: 'Action search on Actions stands in for the command palette',
		},
		order: 1,
		title: 'Command Palette...',
		when: { not: { key: 'editorSimpleInput' } },
	},
	{
		command: 'workbench.output.action.clearOutput',
		group: '',
		orchbee: { kind: 'excluded', reason: 'The Output panel' },
		order: 0,
		title: 'Clear Output',
	},
];
