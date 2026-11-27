export const enUSCodeEditorMessages = {
	'codeEditor.imagePreview.label': 'Image Preview: {name}',
	'codeEditor.imagePreview.loadFailed': 'An error occurred while loading the image.',
	'codeEditor.imagePreview.openAsText': 'Open in Text Editor',
	'codeEditor.placeholder.binary':
		'The file is not displayed in the text editor because it is either binary or uses an unsupported text encoding.',
	'codeEditor.placeholder.directory':
		'The file is not displayed in the text editor because it is a directory.',
	'codeEditor.placeholder.failed': 'The editor could not be opened due to an unexpected error.',
	'codeEditor.placeholder.notFound':
		'The editor could not be opened because the file was not found.',
	'codeEditor.placeholder.openAnyway': 'Open Anyway',
	'codeEditor.placeholder.retry': 'Try Again',
	'codeEditor.placeholder.tooLarge':
		'The file is not displayed in the text editor because it is very large.',
	'codeEditor.placeholder.unreadable': 'Unable to read file “{name}” that is too large to open.',
} as const;

export type CodeEditorMessageId = keyof typeof enUSCodeEditorMessages;
