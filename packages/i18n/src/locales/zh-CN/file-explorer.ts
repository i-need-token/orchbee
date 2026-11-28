import type { FileExplorerMessageId } from '../en-US';

export const zhCNFileExplorerMessages = {
	'fileExplorer.actions.collapse': '在资源管理器中折叠文件夹',
	'fileExplorer.actions.refresh': '刷新资源管理器',
	'fileExplorer.decorations.symbolicLink': '符号链接',
	'fileExplorer.decorations.unknownType': '未知文件类型',
	'fileExplorer.empty.noDirectory': '尚未配置目录。',
	'fileExplorer.root.unresolved': '无法解析工作区文件夹（{reason}）',
	'fileExplorer.root.unresolvedReason.disconnected': '与这台机器的连接已断开',
	'fileExplorer.root.unresolvedReason.failed': '出现意外错误',
	'fileExplorer.root.unresolvedReason.notDirectory': '它不是文件夹',
	'fileExplorer.root.unresolvedReason.notFound': '它不存在',
	'fileExplorer.root.unresolvedReason.permissionDenied': '没有访问权限',
	'fileExplorer.root.unresolvedReason.timeout': '文件夹响应超时',
	'fileExplorer.root.unresolvedReason.unavailable': '这台机器上的文件暂时无法访问',
	'fileExplorer.root.unresolvedReason.unsupportedLocation': '不支持这类位置',
	'fileExplorer.section.label': '资源管理器分区：{name}',
	'fileExplorer.tree.label': '文件资源管理器',
} as const satisfies Readonly<Record<FileExplorerMessageId, string>>;
