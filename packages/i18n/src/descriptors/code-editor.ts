import { defineMessages } from 'react-intl';

import { enUSCodeEditorMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSCodeEditorMessages);

/** A VS Code editor command bound to a key, as Settings → Keyboard names it. */
const command = (id: Parameters<typeof message>[0]): ReturnType<typeof message> =>
	message(id, 'Name of VS Code’s editor command of the same name in Settings → Keyboard.');

/** The File Bar's editors: text files in Monaco and images in the image preview. */
export const codeEditorMessages = defineMessages({
	actionCategory: message(
		'codeEditor.actions.category',
		'Group in Action search that lists the text editor’s own commands and its font size.',
	),
	addLineComment: command('codeEditor.commands.addLineComment'),
	binary: message(
		'codeEditor.placeholder.binary',
		'Shown in place of a file the text editor does not show because it looks binary, above an Open Anyway button.',
	),
	changeAll: message(
		'codeEditor.menu.changeAll',
		'Item of the text editor’s context menu that selects every occurrence of the word to change them together.',
	),
	command: message(
		'codeEditor.actions.command',
		'An editor command in Action search; {label} is the name the editor gives it, already in the editor’s language.',
	),
	copy: message(
		'codeEditor.menu.copy',
		'Item of the text editor’s context menu that copies the selection.',
	),
	createFoldingRangeFromSelection: command('codeEditor.commands.createFoldingRangeFromSelection'),
	cut: message(
		'codeEditor.menu.cut',
		'Item of the text editor’s context menu that cuts the selection.',
	),
	directory: message(
		'codeEditor.placeholder.directory',
		'Shown in place of the text editor when the path opened is a folder.',
	),
	failed: message(
		'codeEditor.placeholder.failed',
		'Shown in place of the editor when reading the file failed for any other reason, above a Try Again button.',
	),
	foldAll: command('codeEditor.commands.foldAll'),
	foldAllBlockComments: command('codeEditor.commands.foldAllBlockComments'),
	foldAllExcept: command('codeEditor.commands.foldAllExcept'),
	foldAllRegions: command('codeEditor.commands.foldAllRegions'),
	foldLevel: message(
		'codeEditor.commands.foldLevel',
		'Name of VS Code’s Fold Level commands in Settings → Keyboard; {level} is the level, from 1 to 7.',
	),
	foldRecursively: command('codeEditor.commands.foldRecursively'),
	fontZoomIn: message(
		'codeEditor.actions.fontZoomIn',
		'Action that makes the text in every editor larger, without changing the window’s zoom.',
	),
	fontZoomOut: message(
		'codeEditor.actions.fontZoomOut',
		'Action that makes the text in every editor smaller, without changing the window’s zoom.',
	),
	fontZoomReset: message(
		'codeEditor.actions.fontZoomReset',
		'Action that returns the text in every editor to its normal size.',
	),
	imageLoadFailed: message(
		'codeEditor.imagePreview.loadFailed',
		'Shown in place of an image whose content could not be decoded as an image, above an Open in Text Editor button.',
	),
	imagePreview: message(
		'codeEditor.imagePreview.label',
		'Accessible name of an image preview; {name} is the file name.',
	),
	moveSelectionToNextFindMatch: command('codeEditor.commands.moveSelectionToNextFindMatch'),
	notFound: message(
		'codeEditor.placeholder.notFound',
		'Shown in place of the editor when the file no longer exists.',
	),
	openAnyway: message(
		'codeEditor.placeholder.openAnyway',
		'Button that opens a binary or very large file in the text editor despite the message above it.',
	),
	openAsText: message(
		'codeEditor.imagePreview.openAsText',
		'Button under an image that failed to load; reopens the file in the text editor.',
	),
	paste: message(
		'codeEditor.menu.paste',
		'Item of the text editor’s context menu that pastes at the cursor.',
	),
	removeLineComment: command('codeEditor.commands.removeLineComment'),
	removeManualFoldingRanges: command('codeEditor.commands.removeManualFoldingRanges'),
	retry: message(
		'codeEditor.placeholder.retry',
		'Button that reads the file again after a failure.',
	),
	searchActions: message(
		'codeEditor.menu.searchActions',
		'Last item of the text editor’s context menu, where VS Code shows Command Palette; opens Action search on Actions.',
	),
	selectFromAnchorToCursor: command('codeEditor.commands.selectFromAnchorToCursor'),
	setSelectionAnchor: command('codeEditor.commands.setSelectionAnchor'),
	showHover: command('codeEditor.commands.showHover'),
	toggleFold: command('codeEditor.commands.toggleFold'),
	toggleFoldRecursively: command('codeEditor.commands.toggleFoldRecursively'),
	tooLarge: message(
		'codeEditor.placeholder.tooLarge',
		'Shown in place of a file the text editor does not show because it is very large, above an Open Anyway button.',
	),
	trimTrailingWhitespace: command('codeEditor.commands.trimTrailingWhitespace'),
	unfoldAll: command('codeEditor.commands.unfoldAll'),
	unfoldAllExcept: command('codeEditor.commands.unfoldAllExcept'),
	unfoldAllRegions: command('codeEditor.commands.unfoldAllRegions'),
	unfoldRecursively: command('codeEditor.commands.unfoldRecursively'),
	unreadable: message(
		'codeEditor.placeholder.unreadable',
		'Shown when a very large file still could not be read after Open Anyway; {name} is the file name.',
	),
});
