import type { RemoteDirectoryBrowserMessageId } from '../en-US';

export const zhCNRemoteDirectoryBrowserMessages = {
	'remoteDirectoryBrowser.actions.cancel': '取消',
	'remoteDirectoryBrowser.actions.editPath': '编辑文件夹路径',
	'remoteDirectoryBrowser.actions.go': '前往',
	'remoteDirectoryBrowser.actions.useFolder': '使用此文件夹',
	'remoteDirectoryBrowser.empty': '此文件夹中没有可访问的子文件夹。',
	'remoteDirectoryBrowser.failure.busy': '远端数据源正忙，请稍后重新打开此路径。',
	'remoteDirectoryBrowser.failure.invalidPath': '请输入文件夹的绝对路径。',
	'remoteDirectoryBrowser.failure.notDirectory': '该路径不是文件夹。',
	'remoteDirectoryBrowser.failure.notFound': '找不到该文件夹。',
	'remoteDirectoryBrowser.failure.permission': '你无权打开该文件夹。',
	'remoteDirectoryBrowser.failure.timedOut': '远端数据源打开该文件夹时超时。',
	'remoteDirectoryBrowser.failure.unavailable': '无法加载该文件夹。请检查连接后重试。',
	'remoteDirectoryBrowser.listLabel': '文件夹',
	'remoteDirectoryBrowser.loading': '正在加载文件夹…',
	'remoteDirectoryBrowser.locationLabel': '当前远端位置',
	'remoteDirectoryBrowser.path': '文件夹路径',
	'remoteDirectoryBrowser.truncated': '部分文件夹未显示。可输入绝对路径打开其他文件夹。',
} as const satisfies Record<RemoteDirectoryBrowserMessageId, string>;
