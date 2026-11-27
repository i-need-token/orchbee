import { defineMessages } from 'react-intl';

import { enUSCodeEditorMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSCodeEditorMessages);

/** The File Bar's editors, which show files in Monaco. */
export const codeEditorMessages = defineMessages({
	binary: message(
		'codeEditor.placeholder.binary',
		'Shown in place of a file the text editor does not show because it looks binary, above an Open Anyway button.',
	),
	directory: message(
		'codeEditor.placeholder.directory',
		'Shown in place of the text editor when the path opened is a folder.',
	),
	failed: message(
		'codeEditor.placeholder.failed',
		'Shown in place of the editor when reading the file failed for any other reason, above a Try Again button.',
	),
	notFound: message(
		'codeEditor.placeholder.notFound',
		'Shown in place of the editor when the file no longer exists.',
	),
	openAnyway: message(
		'codeEditor.placeholder.openAnyway',
		'Button that opens a binary or very large file in the text editor despite the message above it.',
	),
	retry: message(
		'codeEditor.placeholder.retry',
		'Button that reads the file again after a failure.',
	),
	tooLarge: message(
		'codeEditor.placeholder.tooLarge',
		'Shown in place of a file the text editor does not show because it is very large, above an Open Anyway button.',
	),
	unreadable: message(
		'codeEditor.placeholder.unreadable',
		'Shown when a very large file still could not be read after Open Anyway; {name} is the file name.',
	),
});
