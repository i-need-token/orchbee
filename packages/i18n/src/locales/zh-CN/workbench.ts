import type { WorkbenchMessageId } from '../en-US';

export const zhCNWorkbenchMessages = {
	'workbench.sidebar.label': '工作区导航',
	'workbench.sidebar.actions.hide': '隐藏边栏',
	'workbench.sidebar.actions.show': '显示边栏',
	'workbench.sidebar.items.newTerminal': '新建终端',
	'workbench.sidebar.items.search': '搜索',
	'workbench.sidebar.items.settings': '设置',
	'workbench.sidebar.sections.project': '项目',
} as const satisfies Readonly<Record<WorkbenchMessageId, string>>;
