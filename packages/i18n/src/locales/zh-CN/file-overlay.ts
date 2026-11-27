import type { FileOverlayMessageId } from '../en-US';

export const zhCNFileOverlayMessages = {
	'fileOverlay.actions.expand': '展开',
	'fileOverlay.actions.restore': '还原',
	'fileOverlay.actions.retry': '重试',
	'fileOverlay.actions.toggleFull': '展开 / 还原文件浮窗',
	'fileOverlay.label': '文件浮窗',
	'fileOverlay.loadFailed': '没能加载文件浮窗。',
} as const satisfies Readonly<Record<FileOverlayMessageId, string>>;
