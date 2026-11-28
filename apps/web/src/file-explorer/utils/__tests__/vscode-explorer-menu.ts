/**
 * VS Code's Explorer context menu, `MenuId.ExplorerContext` with the built-in extensions'
 * `explorer/context` entries, transcribed from Visual Studio Code (Code - OSS) at commit
 * ef5ca45a6e545e4767e0117905ee9d6120b86e11. The Agents window's own entries are left out, since
 * they show only there.
 *
 * The items stand in VS Code's menu order. An item Orchbee offers names its Action and keeps VS
 * Code's `when` over the context keys below; every other item names the reason it is left out or
 * the slice that adds it. Reveal in OS takes its Windows title, which sorts it as on a Mac; on Linux
 * VS Code names it Open Containing Folder, which sorts it before Open With.
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

/**
 * The context keys the offered items read, by VS Code's names. Each is about the item the menu is
 * for: the row right-clicked, or the root on the empty space below the rows.
 */
type VSCodeContextKey = 'explorerResourceIsFolder' | 'isFileSystemResource' | 'resourceSet';

export type VSCodeCondition =
	| { readonly and: readonly VSCodeCondition[] }
	| { readonly key: VSCodeContextKey }
	| { readonly not: VSCodeCondition };

export interface VSCodeMenuItem {
	readonly command: string;
	readonly group: string;
	readonly orchbee:
		| { readonly action: string; readonly kind: 'offered' }
		| { readonly kind: 'excluded'; readonly reason: string }
		| { readonly kind: 'pending'; readonly slice: string };
	/** VS Code sorts an item without an order as 0. */
	readonly order: number;
	readonly precondition?: VSCodeCondition;
	readonly title: string;
	readonly when?: VSCodeCondition;
}

const NO_FILE_OPERATIONS_YET = 'W4';
const NO_TEST_PROVIDERS = 'Orchbee runs no test providers.';
const NO_LANGUAGE_SERVER = 'Orchbee runs no language server that finds file references.';
const ONE_FOLDER =
	'The Explorer shows one folder, the Workspace directory; it has no multi-root workspace.';
const NO_AGENT_INTEGRATION = 'Orchbee integrates with no Agent for now.';

export const VSCODE_EXPLORER_CONTEXT: readonly VSCodeMenuItem[] = [
	{
		command: 'markdown.showPreview',
		group: 'navigation',
		orchbee: { kind: 'pending', slice: 'W6' },
		order: 0,
		title: 'Open Preview',
	},
	{
		command: 'explorer.newFile',
		group: 'navigation',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 4,
		title: 'New File...',
	},
	{
		command: 'explorer.newFolder',
		group: 'navigation',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 6,
		title: 'New Folder...',
	},
	{
		command: 'explorer.openToSide',
		group: 'navigation',
		orchbee: { action: 'file-explorer.file.open-to-side', kind: 'offered' },
		order: 10,
		title: 'Open to the Side',
		when: { and: [{ not: { key: 'explorerResourceIsFolder' } }, { key: 'resourceSet' }] },
	},
	{
		command: 'explorer.openWith',
		group: 'navigation',
		orchbee: { kind: 'pending', slice: 'M2' },
		order: 20,
		title: 'Open With...',
	},
	{
		command: 'revealFileInOS',
		group: 'navigation',
		orchbee: { kind: 'pending', slice: 'E3' },
		order: 20,
		title: 'Reveal in File Explorer',
	},
	{
		command: 'workbench.action.openImagesInCarousel',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'The owner left the images preview out.' },
		order: 25,
		title: 'Open in Images Preview',
	},
	{
		command: 'workbench.action.browser.openFile',
		group: 'navigation',
		orchbee: { kind: 'excluded', reason: 'The owner left the integrated browser out.' },
		order: 29,
		title: 'Open in Integrated Browser',
	},
	{
		command: 'openInIntegratedTerminal',
		group: 'navigation',
		orchbee: { action: 'file-explorer.terminal.open', kind: 'offered' },
		order: 30,
		title: 'Open in Integrated Terminal',
		// VS Code's condition, a `file` resource at the default `terminal.explorerKind` or a remote one,
		// holds for each resource of the file system its Explorer shows.
		when: { key: 'isFileSystemResource' },
	},
	{
		command: 'openInTerminal',
		group: 'navigation',
		orchbee: {
			kind: 'excluded',
			reason: 'The owner left the external terminal out; VS Code hides it by default too.',
		},
		order: 31,
		title: 'Open in External Terminal',
	},
	{
		command: 'MenuId.ExplorerContextShare',
		group: '11_share',
		orchbee: {
			kind: 'excluded',
			reason: 'The submenu holds only Copy vscode.dev Link, which the owner left out.',
		},
		order: -1,
		title: 'Share',
	},
	{
		command: 'npm.runScriptFromFolder',
		group: '2_workspace',
		orchbee: {
			kind: 'excluded',
			reason: 'Orchbee runs no npm tasks; VS Code hides it unless a setting turns it on.',
		},
		order: 0,
		title: 'Run NPM Script in Folder...',
	},
	{
		command: 'addRootFolder',
		group: '2_workspace',
		orchbee: { kind: 'excluded', reason: ONE_FOLDER },
		order: 10,
		title: 'Add Folder to Workspace...',
	},
	{
		command: '_workbench.action.openFolderSettings',
		group: '2_workspace',
		orchbee: { kind: 'excluded', reason: 'Orchbee has no settings for one folder.' },
		order: 20,
		title: 'Open Folder Settings',
	},
	{
		command: 'removeRootFolder',
		group: '2_workspace',
		orchbee: { kind: 'excluded', reason: ONE_FOLDER },
		order: 30,
		title: 'Remove Folder from Workspace',
	},
	{
		command: 'compareFiles',
		group: '3_compare',
		orchbee: { kind: 'pending', slice: 'M6' },
		order: 20,
		title: 'Compare with Selected',
	},
	{
		command: 'compareSelected',
		group: '3_compare',
		orchbee: { kind: 'pending', slice: 'M6' },
		order: 30,
		title: 'Compare Selected',
	},
	{
		command: 'selectForCompare',
		group: '3_compare',
		orchbee: { kind: 'pending', slice: 'M6' },
		order: 30,
		title: 'Select for Compare',
	},
	{
		command: 'markdown.findAllFileReferences',
		group: '4_search',
		orchbee: { kind: 'excluded', reason: NO_LANGUAGE_SERVER },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'typescript.findAllFileReferences',
		group: '4_search',
		orchbee: { kind: 'excluded', reason: NO_LANGUAGE_SERVER },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'typescript.findAllFileReferences',
		group: '4_search',
		orchbee: { kind: 'excluded', reason: NO_LANGUAGE_SERVER },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'typescript.findAllFileReferences',
		group: '4_search',
		orchbee: { kind: 'excluded', reason: NO_LANGUAGE_SERVER },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'typescript.findAllFileReferences',
		group: '4_search',
		orchbee: { kind: 'excluded', reason: NO_LANGUAGE_SERVER },
		order: 0,
		title: 'Find File References',
	},
	{
		command: 'filesExplorer.findInFolder',
		group: '4_search',
		orchbee: { kind: 'pending', slice: 'R1' },
		order: 10,
		title: 'Find in Folder...',
	},
	{
		command: 'filesExplorer.findInWorkspace',
		group: '4_search',
		orchbee: {
			kind: 'excluded',
			reason:
				'VS Code shows it only for a root that is a file, which the Workspace directory never is.',
		},
		order: 10,
		title: 'Find in Workspace...',
	},
	{
		command: 'files.openTimeline',
		group: '4_timeline',
		orchbee: { kind: 'pending', slice: 'H1' },
		order: 1,
		title: 'Open Timeline',
	},
	{
		command: 'workbench.action.chat.attachFile',
		group: '5_chat',
		orchbee: { kind: 'excluded', reason: NO_AGENT_INTEGRATION },
		order: 1,
		title: 'Add File to Chat',
	},
	{
		command: 'workbench.action.chat.attachFolder',
		group: '5_chat',
		orchbee: { kind: 'excluded', reason: NO_AGENT_INTEGRATION },
		order: 1,
		title: 'Add Folder to Chat',
	},
	{
		command: 'filesExplorer.cut',
		group: '5_cutcopypaste',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 8,
		title: 'Cut',
	},
	{
		command: 'filesExplorer.copy',
		group: '5_cutcopypaste',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 10,
		title: 'Copy',
	},
	{
		command: 'filesExplorer.paste',
		group: '5_cutcopypaste',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 20,
		title: 'Paste',
	},
	{
		command: 'explorer.download',
		group: '5b_importexport',
		orchbee: { kind: 'pending', slice: 'W5' },
		order: 10,
		title: 'Download...',
	},
	{
		command: 'explorer.upload',
		group: '5b_importexport',
		orchbee: { kind: 'pending', slice: 'W5' },
		order: 20,
		title: 'Upload...',
	},
	{
		command: 'copyFilePath',
		group: '6_copypath',
		orchbee: { action: 'file-explorer.path.copy', kind: 'offered' },
		order: 10,
		title: 'Copy Path',
		when: { key: 'isFileSystemResource' },
	},
	{
		command: 'copyRelativeFilePath',
		group: '6_copypath',
		orchbee: { action: 'file-explorer.relative-path.copy', kind: 'offered' },
		order: 20,
		title: 'Copy Relative Path',
		when: { key: 'isFileSystemResource' },
	},
	{
		command: 'testing.run.uri',
		group: '6.5_testing',
		orchbee: { kind: 'excluded', reason: NO_TEST_PROVIDERS },
		order: 11.1,
		title: 'Run Tests',
	},
	{
		command: 'testing.debug.uri',
		group: '6.5_testing',
		orchbee: { kind: 'excluded', reason: NO_TEST_PROVIDERS },
		order: 12.1,
		title: 'Debug Tests',
	},
	{
		command: 'testing.coverage.uri',
		group: '6.5_testing',
		orchbee: { kind: 'excluded', reason: NO_TEST_PROVIDERS },
		order: 12.1,
		title: 'Run Tests with Coverage',
	},
	{
		command: 'renameFile',
		group: '7_modification',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 10,
		title: 'Rename...',
	},
	{
		command: 'moveFileToTrash',
		group: '7_modification',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 20,
		title: 'Delete',
	},
	{
		command: 'deleteFile',
		group: '7_modification',
		orchbee: { kind: 'pending', slice: NO_FILE_OPERATIONS_YET },
		order: 20,
		title: 'Delete Permanently',
	},
	{
		command: 'github.copilot.chat.copilotCLI.addFileReference',
		group: 'copilot',
		orchbee: { kind: 'excluded', reason: NO_AGENT_INTEGRATION },
		order: 0,
		title: 'Add File to Copilot CLI',
	},
	{
		command: 'workbench.extensions.command.installFromVSIX',
		group: 'extensions',
		orchbee: { kind: 'excluded', reason: 'Orchbee runs no VS Code extensions.' },
		order: 0,
		title: 'Install Extension VSIX',
	},
];
