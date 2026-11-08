import type { DesktopDiagnosticsMessageId } from '../en-US';

export const zhCNDesktopDiagnosticsMessages = {
	'desktop.diagnostics.action.saveBundle': '保存诊断包…',
	'desktop.diagnostics.bundleFailed': '诊断包未能保存。',
	'desktop.diagnostics.bundleFailedDetail':
		'未上传任何内容。如需了解原因，请在终端运行 "orchbee diagnostics create" 命令。',
	'desktop.diagnostics.bundlePending': '正在保存诊断包。',
	'desktop.diagnostics.category': '帮助',
	'desktop.diagnostics.close': '关闭',
	'desktop.diagnostics.saveDialogTitle': '保存诊断包',
	'desktop.diagnostics.taskDescription':
		'将版本信息、不含机密的设置和最近 7 天的日志保存到你选择的 ZIP 文件。',
	'desktop.diagnostics.zipFilter': 'ZIP 压缩包',
} as const satisfies Readonly<Record<DesktopDiagnosticsMessageId, string>>;
