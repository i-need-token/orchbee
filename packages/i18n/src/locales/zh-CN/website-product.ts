import type { WebsiteProductMessageId } from '../en-US';

export const zhCNWebsiteProductMessages = {
	'website.product.terminalInterface': 'Orchbee 终端界面',
	'website.product.desktopApplication': 'Orchbee 桌面应用',
	'website.product.commandLine': '运行 orchbee 命令的终端',
	'website.product.summary.attention': '{count} 个需要处理',
	'website.product.summary.working': '{count} 个运行中',
	'website.product.summary.done': '{count} 个完成',
	'website.product.summary.ready': '{count} 个就绪',
} as const satisfies Readonly<Record<WebsiteProductMessageId, string>>;
