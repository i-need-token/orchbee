import type { WebsiteHomeMessageId } from '../en-US';

export const zhCNWebsiteHomeMessages = {
	'website.home.metaTitle': 'Orchbee — 运行和编排 AI Agent 的现代多路复用终端',
	'website.home.metaDescription':
		'会话在后台持续运行，可同时从终端界面、桌面应用和命令行访问，为人与 Agent 提供共同的工作环境。',
	'website.home.title': '一个运行和编排 AI\u00A0Agent 的现代多路复用终端。',
	'website.home.lead':
		'会话在后台持续运行，可同时从终端界面、桌面应用和命令行访问，为人与 Agent 提供共同的工作环境。',
	'website.home.beta': '测试版。目前支持 Windows 11（x86_64）；macOS 和 Linux 版本正在开发中。',
	'website.home.stage.view': '查看这个会话的客户端',
	'website.home.stage.closeHint': '关闭窗口不会结束会话。',
	'website.home.stage.close': '关闭窗口',
	'website.home.stage.awayTitle': '会话正在后台运行。',
	'website.home.stage.awayBody': '没有客户端接入时，其中的 Shell 与 Agent 照常运行。',
	'website.home.stage.closedFor': '窗口已关闭',
	'website.home.stage.reattach': '重新接入',
	'website.home.stage.caption':
		'<b>演示</b>一个会话，里面有一个 Shell 和两个 Agent；画面按真实界面绘制，按脚本播放。',
	'website.home.clients.kicker': '客户端',
	'website.home.clients.title': '各客户端详解',
	'website.home.clients.lead': '终端界面、桌面应用与命令行呈现并控制同一个会话，各有一页单独介绍。',
	'website.home.install.kicker': '测试版',
	'website.home.install.title': '安装',
	'website.home.install.lead':
		'Orchbee 处于测试阶段，目前支持 Windows 11（x86_64）；macOS 和 Linux 版本正在开发中。',
	'website.home.install.releaseNotes': '{version} · 发布说明',
	'website.home.install.app': '桌面应用、终端和 CLI',
	'website.home.install.npm': '仅终端和 CLI',
	'website.home.install.npmNote': '需要 Node.js 22 或更高版本。安装终端界面和 CLI，不含桌面应用。',
	'website.home.install.start': '启动',
	'website.home.install.startNote': '打开终端界面。也可以从开始菜单打开 Orchbee。',
	'website.home.install.note': '安装程序暂未做代码签名，Windows SmartScreen 可能会要求你确认。',
	'website.home.install.source': '获取源码并安装依赖',
	'website.home.install.sourceRequirements':
		'Windows 11。需要 Git、Node.js 22+、Corepack、Rust 1.97.0 和 Zig 0.16.0。',
	'website.home.install.sourceStart': '启动桌面应用',
	'website.home.install.sourceNote': '预构建版本还没发布，这里是从源码构建。',
} as const satisfies Readonly<Record<WebsiteHomeMessageId, string>>;
