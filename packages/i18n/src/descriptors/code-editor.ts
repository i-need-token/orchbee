import { defineMessages } from 'react-intl';

import { enUSCodeEditorMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSCodeEditorMessages);

/** The File Bar's editors: text files in Monaco and images in the image preview. */
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
	imageLoadFailed: message(
		'codeEditor.imagePreview.loadFailed',
		'Shown in place of an image whose content could not be decoded as an image, above an Open in Text Editor button.',
	),
	imagePreview: message(
		'codeEditor.imagePreview.label',
		'Accessible name of an image preview; {name} is the file name.',
	),
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
