import type { WebsiteDesktopMessageId } from '../en-US';

export const zhCNWebsiteDesktopMessages = {
	'website.desktop.metaTitle': '桌面应用 — Orchbee',
	'website.desktop.metaDescription':
		'在独立窗口中呈现同一个会话：标签栏、侧边栏，以及可以用鼠标排布的面板。终端使用 WebGPU 渲染。',
	'website.desktop.lead':
		'在独立窗口中呈现同一个会话：标签栏、侧边栏，以及可以用鼠标排布的面板。终端使用 WebGPU 渲染。',
	'website.desktop.appearance': '外观',
	'website.desktop.appearance.dark': '深色',
	'website.desktop.appearance.light': '浅色',
	'website.desktop.appearance.note': '可跟随系统，也可固定为浅色或深色。',
	'website.desktop.stage.caption': '<b>演示</b>按真实窗口的尺寸绘制。',
	'website.desktop.panels.kicker': '面板',
	'website.desktop.panels.title': '用鼠标或快捷键排布面板',
	'website.desktop.panels.lead':
		'向右或向下拆分面板，拖动分隔线，把一个面板最大化再还原。在应用里，把一个面板拖到另一个面板的边缘，就会在那里拆分。',
	'website.desktop.highlights.title': '窗口带来的能力',
	'website.desktop.highlight.tabs.title': '可以整理的标签页',
	'website.desktop.highlight.tabs.body':
		'拖动标签页调整顺序，固定常用的，用九种颜色的分组把相关标签页归在一起。重命名、复制、关闭都在标签页的菜单里。',
	'website.desktop.highlight.sidebar.title': '装下所有项目的侧边栏',
	'website.desktop.highlight.sidebar.body':
		'项目、工作区、Git 工作树，以及其中的 Agent 和各自的状态。宽度可以在 200 到 400 px 之间拖动，也可以收起。',
	'website.desktop.highlight.search.title': '什么都能搜',
	'website.desktop.highlight.search.body':
		'<code>Ctrl+Shift+P</code> 搜索操作。<code>Ctrl+Shift+K</code> 一次搜索所有内容：Agent、操作和设置。',
	'website.desktop.highlight.search.bodyMacos':
		'<code>Ctrl+Shift+P</code> 搜索操作。<code>Ctrl+Shift+K</code> 一次搜索所有内容：Agent、操作和设置。在 Mac 上分别是 <code>⇧⌘P</code> 和 <code>⇧⌘F</code>。',
	'website.desktop.highlight.find.title': '在终端里查找',
	'website.desktop.highlight.find.body':
		'在面板里查找文本，可以区分大小写、全字匹配、使用正则表达式。',
	'website.desktop.highlight.quickSelect.title': '快速选择',
	'website.desktop.highlight.quickSelect.body':
		'标出屏幕上的每个 URL、路径、<code>file:line</code> 和提交哈希，用键盘选一个：URL 会被打开，路径或哈希会插入到提示符处。',
	'website.desktop.highlight.agents.title': '在你想要的位置启动 Agent',
	'website.desktop.highlight.agents.body':
		'询问启动哪个 Agent、放在哪里：新标签页、向右拆分、向下拆分，或是新的工作树。',
	'website.desktop.highlight.settings.title': '一页看完的设置',
	'website.desktop.highlight.settings.body':
		'在工作区上方打开的大对话框：八个分类、一个搜索框，每项设置都可以恢复默认。',
	'website.desktop.highlight.appearance.title': '浅色和深色，各选各的',
	'website.desktop.highlight.appearance.body':
		'外观可以跟随系统，也可以固定为浅色或深色。终端主题为两者分别选择，共 16 个内置主题。',
	'website.desktop.highlight.input.title': '文字照常输入',
	'website.desktop.highlight.input.body':
		'支持输入法组合输入、连字、Kitty 图形协议和可以点击的超链接。内置 JetBrains Mono 和 Nerd Font 符号，不用另装字体。',
	'website.desktop.background.kicker': '后台',
	'website.desktop.background.title': '关闭窗口不等于退出',
	'website.desktop.background.body':
		'不管窗口开着还是关着，Orchbee 的图标都在通知区域。只要还有终端在运行，关闭窗口后 Orchbee 会继续在那里运行。图标显示最紧急的 Agent 状态，菜单里写着有多少终端正在运行。',
	'website.desktop.background.bodyWindows':
		'在 Windows 上，不管窗口开着还是关着，Orchbee 的图标都在通知区域。只要还有终端在运行，关闭窗口后 Orchbee 会继续在那里运行。图标显示最紧急的 Agent 状态，菜单里写着有多少终端正在运行。',
	'website.desktop.background.macos':
		'在 Mac 上，同样的菜单在菜单栏的 Orchbee 图标里；关闭窗口后 Orchbee 会留在程序坞中，不管有没有终端在运行。',
	'website.desktop.background.quitAll': '“全部退出”在停止任何内容之前会先询问。',
	'website.desktop.tray.label': 'Orchbee 的 Windows 通知和通知区域菜单',
	'website.desktop.tray.whenClosed': '关闭窗口时',
	'website.desktop.tray.whenMenu': '右键点击图标时',
	'website.desktop.rendering.kicker': '渲染',
	'website.desktop.rendering.title': '由 GPU 绘制',
	'website.desktop.rendering.body':
		'桌面应用使用 WebGPU 渲染，并提供 Canvas 2D 后备。连字默认开启，字体为 13 px 的 JetBrains Mono，内置 Nerd Font 符号。',
	'website.desktop.fact.canvas': 'Canvas 2D 后备',
	'website.desktop.fact.color': '24 位真彩色',
	'website.desktop.fact.kitty': 'Kitty 图形协议',
	'website.desktop.fact.history': '每个终端 10 MB 历史输出',
	'website.desktop.ligatures': '连字',
	'website.desktop.ligatures.on': '开',
	'website.desktop.ligatures.off': '关',
	'website.desktop.keys.title': '快捷键',
	'website.desktop.keys.lead':
		'Windows 上的快捷键。每个快捷键对应的操作，都可以用 <code>Ctrl+Shift+P</code> 搜到。',
	'website.desktop.keys.leadMacos':
		'每个快捷键对应的操作，都可以用 <code>Ctrl+Shift+P</code> 搜到，在 Mac 上是 <code>⇧⌘P</code>。',
	'website.desktop.keys.terminals': '终端与标签页',
	'website.desktop.keys.terminals.newTerminal': '新建终端',
	'website.desktop.keys.terminals.newWorkspace': '新建工作区',
	'website.desktop.keys.terminals.startAgent': '启动 Agent',
	'website.desktop.keys.terminals.goToTab': '切换标签页',
	'website.desktop.keys.panels': '面板',
	'website.desktop.keys.panels.splitRight': '向右拆分',
	'website.desktop.keys.panels.splitDown': '向下拆分',
	'website.desktop.keys.panels.maximize': '最大化 / 还原',
	'website.desktop.keys.panels.equalize': '均分面板',
	'website.desktop.keys.panels.focus': '移动焦点',
	'website.desktop.keys.panels.resize': '调整大小',
	'website.desktop.keys.search': '搜索',
	'website.desktop.keys.search.actions': '搜索操作',
	'website.desktop.keys.search.everything': '搜索所有内容',
	'website.desktop.keys.search.find': '在终端里查找',
	'website.desktop.keys.search.quickSelect': '快速选择',
	'website.desktop.keys.window': '窗口',
	'website.desktop.keys.window.settings': '设置',
	'website.desktop.keys.window.sidebar': '显示或收起侧边栏',
} as const satisfies Readonly<Record<WebsiteDesktopMessageId, string>>;
