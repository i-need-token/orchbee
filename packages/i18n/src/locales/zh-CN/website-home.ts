import type { WebsiteHomeMessageId } from '../en-US';

export const zhCNWebsiteHomeMessages = {
	'website.home.metaTitle': 'Orchbee — 运行和编排 AI Agent 的现代多路复用终端',
	'website.home.metaDescription':
		'并排运行 Claude Code、Codex、Gemini CLI 等 30 多种 Agent。哪个需要你一眼就能看到，关掉窗口它们也照常运行。免费开源。',
	'website.home.title': '一个运行和编排 AI\u00A0Agent 的现代多路复用终端。',
	'website.home.lead':
		'并排运行 Claude Code、Codex、Gemini CLI 等 30 多种 Agent。哪个需要你一眼就能看到；关掉窗口，它们照常运行；Agent 之间还能互相派活。',
	'website.home.beta': '测试版',
	'website.home.facts': '免费<repo>开源</repo> · Windows · 不做遥测',
	'website.home.factsMacos': '免费<repo>开源</repo> · Windows 与 macOS · 不做遥测',
	'website.home.factsLinux': '免费<repo>开源</repo> · Windows 与 Linux · 不做遥测',
	'website.home.factsAll': '免费<repo>开源</repo> · Windows、macOS 与 Linux · 不做遥测',
	'website.home.stage.view': '查看这个会话的客户端',
	'website.home.stage.closeHint': '关掉窗口，Agent 照常运行。',
	'website.home.stage.close': '关闭窗口',
	'website.home.stage.awayTitle': '会话正在后台运行。',
	'website.home.stage.awayBody': '没有客户端接入时，其中的 Shell 与 Agent 照常运行。',
	'website.home.stage.closedFor': '窗口已关闭',
	'website.home.stage.reattach': '重新接入',
	'website.home.stage.caption': '<b>演示</b>一个会话里运行着一个 Shell 和两个 Agent，按脚本播放。',
	'website.home.benefits.kicker': '优势',
	'website.home.benefits.title': '为同时运行多个 Agent 而生',
	'website.home.benefits.lead':
		'Orchbee 不替代你的 Agent，而是给它们一个可以并排运行、互相协作的终端。你的角色从 <b>human-in-the-loop</b> 变成 <b>human-on-the-loop</b>：只需监督全局，在必要时介入。',
	'website.home.benefits.statusTitle': '所有 Agent，一眼看清',
	'website.home.benefits.statusBody':
		'侧边栏列出所有项目里的每个 Agent 及其状态：工作中、等你处理、已完成或空闲。不用逐个打开终端，就知道该看哪一个。',
	'website.home.benefits.persistTitle': '关掉窗口，工作照常',
	'website.home.benefits.persistBody':
		'窗口关闭后，会话连同其中的 Shell 和 Agent 继续在后台运行。之后可以从桌面应用、终端界面或命令行重新接入，三者也能同时接入。',
	'website.home.benefits.delegateTitle': 'Agent 之间互相派活',
	'website.home.benefits.delegateBody':
		'借助 <code>orchbee</code> 命令，一个 Agent 可以启动另一个 Agent、给它发 prompt、等它完成并读取输出。比如让 Claude Code 把任务交给 Codex，你在一旁看着就行。',
	'website.home.benefits.terminalTitle': '真正的终端内核',
	'website.home.benefits.terminalBody':
		'基于 Ghostty 核心构建，支持 24 位真彩色、连字、Kitty 图形协议、可点击链接和输入法组合输入，内置 {count} 个主题。桌面应用使用 WebGPU 渲染。',
	'website.home.agents.kicker': 'agent',
	'website.home.agents.title': '支持你正在用的 Agent',
	'website.home.agents.lead':
		'Orchbee 可识别 30 多种 CLI Agent，直接从屏幕上读取每个 Agent 的状态，无需在 Agent 里安装或配置任何东西。',
	'website.home.agents.all': '查看全部 {count} 种可识别的 Agent',
	'website.home.clients.kicker': '客户端',
	'website.home.clients.title': '一个会话，三种接入方式',
	'website.home.clients.lead': '桌面应用、终端界面和命令行呈现并控制同一个会话，按场景选用即可。',
	'website.home.faq.kicker': '问答',
	'website.home.faq.title': '常见问题',
	'website.home.faq.freeQuestion': 'Orchbee 免费吗？',
	'website.home.faq.freeAnswer': '免费。Orchbee 以 MIT 许可证开源，源码在 <repo>GitHub</repo> 上。',
	'website.home.faq.replaceQuestion': '它会取代 Claude Code 或 Codex 吗？',
	'website.home.faq.replaceAnswer':
		'不会。你继续使用现有的 Agent、账号和设置。Orchbee 是它们运行的终端：显示它们的状态，让它们持续运行，并让它们协同工作。',
	'website.home.faq.tmuxQuestion': '它和 tmux 有什么不同？',
	'website.home.faq.tmuxAnswer':
		'和 tmux 一样，Orchbee 让会话在后台持续运行，并能把会话分成多个面板。此外，它能识别哪些程序是 Agent、各自处于什么状态，让 Agent 通过命令行互相驱动，而且除了终端界面还提供桌面应用。',
	'website.home.faq.platformsQuestion': '支持哪些系统？',
	'website.home.faq.platformsAnswer': '目前支持 Windows（x86_64），macOS 和 Linux 版本正在开发中。',
	'website.home.faq.platformsAnswerMacos':
		'目前支持 Windows（x86_64）和 macOS 13 或更高版本，Linux 版本正在开发中。',
	'website.home.faq.platformsAnswerLinux':
		'目前支持 Windows（x86_64）和 Linux（x86_64 与 ARM64），macOS 版本正在开发中。',
	'website.home.faq.platformsAnswerAll':
		'支持 Windows（x86_64）、macOS 13 或更高版本，以及 Linux（x86_64 与 ARM64）。',
	'website.home.faq.dataQuestion': 'Orchbee 会收集我的数据吗？',
	'website.home.faq.dataAnswer':
		'不会。Orchbee 现在和以后都不做遥测：不上传使用数据，不追踪你的操作，也不自动上传崩溃报告；匿名遥测和可选遥测同样不做。',
	'website.home.faq.nextQuestion': '接下来会做什么？',
	'website.home.faq.nextAnswer':
		'远程机器。本机资源不够时，可以通过 SSH 添加其他电脑，Agent 就能用上它们的算力，用法不变。',
	'website.home.install.kicker': '测试版',
	'website.home.install.title': '安装',
	'website.home.install.lead':
		'Orchbee 处于测试阶段，目前支持 Windows（x86_64）；macOS 和 Linux 版本正在开发中。',
	'website.home.install.releaseNotes': '{version} · 发布说明',
	'website.home.install.app': '桌面应用、终端和 CLI',
	'website.home.install.npm': '仅终端和 CLI',
	'website.home.install.npmNote': '需要 Node.js 22 或更高版本。安装终端界面和 CLI，不含桌面应用。',
	'website.home.install.start': '启动',
	'website.home.install.startNote': '打开终端界面。也可以从开始菜单打开 Orchbee。',
	'website.home.install.leadMacos':
		'Orchbee 处于测试阶段，目前支持 Windows（x86_64）和 macOS 13 或更高版本；Linux 版本正在开发中。',
	'website.home.install.leadLinux':
		'Orchbee 处于测试阶段，目前支持 Windows（x86_64）和 Linux（x86_64 与 ARM64）；macOS 版本正在开发中。',
	'website.home.install.leadAll':
		'Orchbee 处于测试阶段，支持 Windows（x86_64）、macOS 13 或更高版本，以及 Linux（x86_64 与 ARM64）。',
	'website.home.install.platform': '平台',
	'website.home.install.tabNpm': 'npm（仅 CLI）',
	'website.home.install.startNoteMacos':
		'打开终端界面。也可以从 Windows 的开始菜单或 Mac 的“应用程序”中打开 Orchbee。',
	'website.home.install.startNoteLinux':
		'打开终端界面。也可以从 Windows 的开始菜单或 Linux 的应用程序菜单中打开 Orchbee。',
	'website.home.install.startNoteAll':
		'打开终端界面。也可以从 Windows 的开始菜单、Mac 的“应用程序”或 Linux 的应用程序菜单中打开 Orchbee。',
	'website.home.install.source': '获取源码并安装依赖',
	'website.home.install.sourceRequirements':
		'Windows（x86_64）。需要 Git、Node.js 22+、Corepack、Rust 1.97.0 和 Zig 0.16.0。',
	'website.home.install.sourceStart': '启动桌面应用',
	'website.home.install.sourceNote': '预构建版本还没发布，这里是从源码构建。',
} as const satisfies Readonly<Record<WebsiteHomeMessageId, string>>;
