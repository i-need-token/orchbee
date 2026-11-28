import type { FileBarMessageId } from '../en-US';

export const zhCNFileBarMessages = {
	'fileBar.actions.hide': '隐藏文件栏',
	'fileBar.actions.show': '显示文件栏',
	'fileBar.actions.showExplorer': '显示资源管理器',
	'fileBar.actions.showSourceControl': '显示源代码管理',
	'fileBar.actions.toggle': '切换文件栏',
	'fileBar.badge.labelled': '{label} - {badge}',
	'fileBar.badge.overThousands': '{count}K+',
	'fileBar.badge.thousands': '{count}K',
	'fileBar.label': '文件栏',
	'fileBar.loadFailed': '没能加载文件栏。',
	'fileBar.tree.compactItem': '{name}，紧凑，{parents}',
	'fileBar.views.explorer': '资源管理器',
	'fileBar.views.sourceControl': '源代码管理',
	'fileBar.viewSwitcher': '活动视图切换器',
} as const satisfies Readonly<Record<FileBarMessageId, string>>;
