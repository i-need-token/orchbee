import type { FileBarMessageId } from '../en-US';

export const zhCNFileBarMessages = {
	'fileBar.actions.toggle': '切换文件栏',
	'fileBar.label': '文件栏',
	'fileBar.loadFailed': '没能加载文件栏。',
} as const satisfies Readonly<Record<FileBarMessageId, string>>;
