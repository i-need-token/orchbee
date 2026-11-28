/**
 * VS Code's tab menu, `MenuId.EditorTitleContext` with the built-in extensions'
 * `editor/title/context` entries, transcribed from Visual Studio Code (Code - OSS) at commit
 * ef5ca45a6e545e4767e0117905ee9d6120b86e11.
 *
 * The items stand in VS Code's menu order. An item Orchbee offers names its Action and keeps VS
 * Code's `when` and `precondition` over the context keys below; every other item names the reason
 * it is left out or the slice that adds it.
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

/** The context keys the tab menu reads, by VS Code's names; each is about the tab right-clicked. */
type VSCodeContextKey =
	| 'activeEditorCannotClose'
	| 'activeEditorIsLastInGroup'
	| 'activeEditorIsNotPreview'
	| 'activeEditorIsPinned'
	| 'config.workbench.editor.enablePreview'
	| 'editorTabsVisible'
	| 'multipleEditorsSelectedInGroup';

export type VSCodeCondition =
	| { readonly and: readonly VSCodeCondition[] }
	| { readonly key: 'groupEditorsCount'; readonly notEquals: number }
	| { readonly key: VSCodeContextKey }
	| { readonly not: VSCodeCondition };

export interface VSCodeMenuItem {
	readonly command: string;
	readonly group: string;
	readonly orchbee:
		| { readonly action: string; readonly kind: 'offered' }
		| { readonly kind: 'excluded'; readonly reason: string }
		| { readonly kind: 'pending'; readonly slice: string };
	readonly order: number;
	readonly precondition?: VSCodeCondition;
	readonly title: string;
	readonly when?: VSCodeCondition;
}

export const VSCODE_EDITOR_TITLE_CONTEXT: readonly VSCodeMenuItem[] = [
	{
		command: 'workbench.action.closeActiveEditor',
		group: '1_close',
		orchbee: { action: 'file-overlay.tab.close', kind: 'offered' },
		order: 10,
		title: 'Close',
		when: { not: { key: 'activeEditorCannotClose' } },
	},
	{
		command: 'workbench.action.closeOtherEditors',
		group: '1_close',
		orchbee: { action: 'file-overlay.tab.close-others', kind: 'offered' },
		order: 20,
		precondition: { key: 'groupEditorsCount', notEquals: 1 },
		title: 'Close Others',
	},
	{
		command: 'workbench.action.closeEditorsToTheRight',
		group: '1_close',
		orchbee: { action: 'file-overlay.tab.close-right', kind: 'offered' },
		order: 30,
		precondition: {
			and: [
				{ not: { key: 'activeEditorIsLastInGroup' } },
				{ not: { key: 'multipleEditorsSelectedInGroup' } },
			],
		},
		title: 'Close to the Right',
		when: { key: 'editorTabsVisible' },
	},
	{
		command: 'workbench.action.closeUnmodifiedEditors',
		group: '1_close',
		orchbee: { action: 'file-overlay.tab.close-saved', kind: 'offered' },
		order: 40,
		title: 'Close Saved',
	},
	{
		command: 'workbench.action.closeEditorsInGroup',
		group: '1_close',
		orchbee: { action: 'file-overlay.tab.close-all', kind: 'offered' },
		order: 50,
		title: 'Close All',
	},
	{
		command: 'workbench.action.browser.closeAllInGroup',
		group: '1_close',
		orchbee: { kind: 'excluded', reason: 'The Integrated Browser' },
		order: 55,
		title: 'Close All Browser Tabs',
	},
	{
		command: 'workbench.action.files.save',
		group: '1_close_save',
		orchbee: { kind: 'pending', slice: 'W1' },
		order: 10,
		title: 'Save',
	},
	{
		command: 'workbench.action.files.saveAs',
		group: '1_close_save',
		orchbee: { kind: 'pending', slice: 'W3' },
		order: 20,
		title: 'Save As...',
	},
	{
		command: 'compareSelected',
		group: '1_compare',
		orchbee: { kind: 'excluded', reason: 'Selecting several tabs at once' },
		order: 30,
		title: 'Compare Selected',
	},
	{
		command: 'workbench.action.chat.copySessionLink',
		group: '1_cutcopypaste',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 0,
		title: 'Copy Link',
	},
	{
		command: 'copyFilePath',
		group: '1_cutcopypaste',
		orchbee: { kind: 'pending', slice: 'E3' },
		order: 0,
		title: 'Copy Path',
	},
	{
		command: 'copyRelativeFilePath',
		group: '1_cutcopypaste',
		orchbee: { kind: 'pending', slice: 'E3' },
		order: 0,
		title: 'Copy Relative Path',
	},
	{
		command: 'breadcrumbs.copyPath',
		group: '1_cutcopypaste',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 100,
		title: 'Copy Breadcrumbs Path',
	},
	{
		command: 'markdown.showPreview',
		group: '1_open',
		orchbee: { kind: 'pending', slice: 'W6' },
		order: 0,
		title: 'Open Preview',
	},
	{
		command: 'workbench.action.browser.openFile',
		group: '1_open',
		orchbee: { kind: 'excluded', reason: 'The Integrated Browser' },
		order: 5,
		title: 'Open in Integrated Browser',
	},
	{
		command: 'workbench.action.reopenWithEditor',
		group: '1_open',
		orchbee: { kind: 'excluded', reason: 'A file opens in one editor only' },
		order: 10,
		title: 'Reopen Editor With...',
	},
	{
		command: 'workbench.action.browser.duplicateTab',
		group: '1_open',
		orchbee: { kind: 'excluded', reason: 'The Integrated Browser' },
		order: 20,
		title: 'Duplicate Tab',
	},
	{
		command: 'submenu:EditorTitleContextShare',
		group: '11_share',
		orchbee: { kind: 'excluded', reason: 'Share' },
		order: -1,
		title: 'Share',
	},
	{
		command: 'workbench.action.chat.attachFile',
		group: '2_chat',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 1,
		title: 'Add File to Chat',
	},
	{
		command: 'workbench.action.terminal.changeColor',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'Terminals in the editor area' },
		order: 0,
		title: 'Change Color...',
	},
	{
		command: 'workbench.action.terminal.changeIcon',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'Terminals in the editor area' },
		order: 0,
		title: 'Change Icon...',
	},
	{
		command: 'workbench.action.terminal.moveToTerminalPanel',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'Terminals in the editor area' },
		order: 0,
		title: 'Move Terminal into Panel',
	},
	{
		command: 'workbench.action.terminal.rename',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'Terminals in the editor area' },
		order: 0,
		title: 'Rename...',
	},
	{
		command: 'revealFileInOS',
		group: '2_files',
		orchbee: { kind: 'pending', slice: 'E3' },
		order: 0,
		title: 'Reveal in File Explorer',
	},
	{
		command: 'workbench.action.terminal.sizeToContentWidth',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'Terminals in the editor area' },
		order: 0,
		title: 'Toggle Size to Content Width',
	},
	{
		command: 'revealInExplorer',
		group: '2_files',
		orchbee: { kind: 'pending', slice: 'E3' },
		order: 1,
		title: 'Reveal in Explorer View',
	},
	{
		command: 'revealInExplorer',
		group: '2_files',
		orchbee: { kind: 'excluded', reason: 'The Agent Sessions window' },
		order: 1,
		title: 'Reveal in Files View',
	},
	{
		command: 'workbench.action.keepEditor',
		group: '3_preview',
		orchbee: { action: 'file-overlay.tab.keep', kind: 'offered' },
		order: 10,
		precondition: { not: { key: 'activeEditorIsNotPreview' } },
		title: 'Keep Open',
		when: { key: 'config.workbench.editor.enablePreview' },
	},
	{
		command: 'workbench.action.pinEditor',
		group: '3_preview',
		orchbee: { action: 'file-overlay.tab.pin', kind: 'offered' },
		order: 20,
		title: 'Pin',
		when: { not: { key: 'activeEditorIsPinned' } },
	},
	{
		command: 'workbench.action.unpinEditor',
		group: '3_preview',
		orchbee: { action: 'file-overlay.tab.unpin', kind: 'offered' },
		order: 20,
		title: 'Unpin',
		when: { key: 'activeEditorIsPinned' },
	},
	{
		command: 'workbench.action.addCanvasToSidebar',
		group: '4_canvas',
		orchbee: { kind: 'excluded', reason: 'The Agent Sessions window' },
		order: 10,
		title: 'Add to Sidebar',
	},
	{
		command: 'workbench.action.removeCanvasFromSidebar',
		group: '4_canvas',
		orchbee: { kind: 'excluded', reason: 'The Agent Sessions window' },
		order: 10,
		title: 'Remove from Sidebar',
	},
	{
		command: 'workbench.action.splitEditor',
		group: '5_split',
		orchbee: { kind: 'excluded', reason: 'Multiple editor groups' },
		order: 10,
		title: 'Split Down',
	},
	{
		command: 'workbench.action.splitEditor',
		group: '5_split',
		orchbee: { kind: 'excluded', reason: 'Multiple editor groups' },
		order: 10,
		title: 'Split Right',
	},
	{
		command: 'submenu:EditorSplitMoveSubmenu',
		group: '5_split',
		orchbee: { kind: 'excluded', reason: 'Multiple editor groups' },
		order: 15,
		title: 'Split & Move',
	},
	{
		command: 'workbench.action.moveEditorToNewWindow',
		group: '7_new_window',
		orchbee: { kind: 'excluded', reason: 'Multiple editor groups' },
		order: 10,
		title: 'Move into New Window',
	},
	{
		command: 'workbench.action.copyEditorToNewWindow',
		group: '7_new_window',
		orchbee: { kind: 'excluded', reason: 'Multiple editor groups' },
		order: 20,
		title: 'Copy into New Window',
	},
	{
		command: 'submenu:ChatWindowTerminalPosition',
		group: '9_chatWindow',
		orchbee: { kind: 'excluded', reason: 'The Agent Sessions window' },
		order: 1,
		title: 'Terminal Position',
	},
	{
		command: 'github.copilot.chat.copilotCLI.addFileReference',
		group: 'copilot',
		orchbee: { kind: 'excluded', reason: 'AI items' },
		order: 0,
		title: 'Add File to Copilot CLI',
	},
	{
		command: 'markdown.findAllFileReferences',
		group: '',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'typescript.findAllFileReferences',
		group: '',
		orchbee: { kind: 'excluded', reason: 'Language services' },
		order: 0,
		title: 'Find File References',
	},
];
