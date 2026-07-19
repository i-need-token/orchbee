import type { WorkbenchMessageId } from '../en-US';

export const zhCNWorkbenchMessages = {
	'workbench.sidebar.actions.hide': '隐藏边栏',
	'workbench.sidebar.actions.show': '显示边栏',
	'workbench.tabs.actions.add': '新建标签页',
	'workbench.tabs.actions.close': '关闭“{tabLabel}”',
	'workbench.tabs.label': '工作区标签页',
	'workbench.tabs.untitled': '新标签页 {tabNumber}',
} as const satisfies Readonly<Record<WorkbenchMessageId, string>>;
