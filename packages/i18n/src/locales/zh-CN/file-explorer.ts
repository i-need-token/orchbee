import type { FileExplorerMessageId } from '../en-US';

export const zhCNFileExplorerMessages = {
	'fileExplorer.actions.collapse': '在资源管理器中折叠文件夹',
	'fileExplorer.actions.refresh': '刷新资源管理器',
	'fileExplorer.decorations.symbolicLink': '符号链接',
	'fileExplorer.decorations.unknownType': '未知文件类型',
	'fileExplorer.empty.noDirectory': '尚未配置目录。',
	'fileExplorer.folder.unreadable': '无法读取文件夹 {name}（{reason}）',
	'fileExplorer.progress.label': '正在加载文件夹',
	'fileExplorer.readFailure.disconnected': '与这台机器的连接已断开',
	'fileExplorer.readFailure.failed': '出现意外错误',
	'fileExplorer.readFailure.notDirectory': '它不是文件夹',
	'fileExplorer.readFailure.notFound': '它不存在',
	'fileExplorer.readFailure.permissionDenied': '没有访问权限',
	'fileExplorer.readFailure.timeout': '文件夹响应超时',
	'fileExplorer.readFailure.unavailable': '这台机器上的文件暂时无法访问',
	'fileExplorer.readFailure.unsupportedLocation': '不支持这类位置',
	'fileExplorer.root.unresolved': '无法解析工作区文件夹（{reason}）',
	'fileExplorer.section.label': '资源管理器分区：{name}',
	'fileExplorer.tree.label': '文件资源管理器',
} as const satisfies Readonly<Record<FileExplorerMessageId, string>>;
