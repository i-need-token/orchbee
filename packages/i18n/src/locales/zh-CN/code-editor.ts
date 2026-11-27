import type { CodeEditorMessageId } from '../en-US';

export const zhCNCodeEditorMessages = {
	'codeEditor.imagePreview.label': '图片预览：{name}',
	'codeEditor.imagePreview.loadFailed': '没能加载这张图片。',
	'codeEditor.imagePreview.openAsText': '在文本编辑器中打开',
	'codeEditor.placeholder.binary':
		'这个文件是二进制文件，或者用了不支持的文本编码，所以没有在文本编辑器中显示。',
	'codeEditor.placeholder.directory': '这是一个文件夹，所以没有在文本编辑器中显示。',
	'codeEditor.placeholder.failed': '出现意外错误，没能打开编辑器。',
	'codeEditor.placeholder.notFound': '找不到这个文件，所以没能打开编辑器。',
	'codeEditor.placeholder.openAnyway': '仍然打开',
	'codeEditor.placeholder.retry': '重试',
	'codeEditor.placeholder.tooLarge': '这个文件非常大，所以没有在文本编辑器中显示。',
	'codeEditor.placeholder.unreadable': '文件“{name}”太大，没能读取。',
} as const satisfies Readonly<Record<CodeEditorMessageId, string>>;
