import type { WebMessageId } from '../en-US';

export const zhCNWebMessages = {
	'web.inMemory.commands.help.output': '可用命令：help、status、clear',
	'web.inMemory.commands.simulated.output': '模拟命令：{command}',
	'web.inMemory.commands.status.output':
		'内存会话 · {columnCount, number} 列 × {rowCount, number} 行',
	'web.inMemory.sessions.agentOutput.introduction':
		'Codex Agent · 已就绪\n正在等待 Workbench 第一个垂直切片中的任务。',
	'web.inMemory.sessions.agentOutput.title': 'Agent 输出',
	'web.inMemory.sessions.localShell.introduction':
		'Orchbee Workbench\n此终端由内存会话适配器提供支持。输入 help 开始使用。',
	'web.inMemory.sessions.localShell.title': '本地 Shell',
	'web.inMemory.sessions.new.introduction': '新的内存终端会话。',
	'web.inMemory.sessions.new.title': '会话 {sessionNumber, number}',
} as const satisfies Readonly<Record<WebMessageId, string>>;
