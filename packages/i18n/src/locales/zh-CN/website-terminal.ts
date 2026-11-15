import type { WebsiteTerminalMessageId } from '../en-US';

export const zhCNWebsiteTerminalMessages = {
	'website.terminal.metaTitle': '终端界面 — Orchbee',
	'website.terminal.metaDescription':
		'运行在你已有的终端里，带侧边栏、标签页与分屏面板。它与桌面应用呈现的是同一个会话。',
	'website.terminal.lead':
		'运行在你已有的终端里，带侧边栏、标签页与分屏面板。它与桌面应用呈现的是同一个会话。',
	'website.terminal.note': '然后运行 <code>orchbee</code>。',
	'website.terminal.stage.caption':
		'<b>演示</b>侧边栏、一个标签页和三个面板：一个 Shell 和两个 Agent。',
	'website.terminal.panels.kicker': '面板',
	'website.terminal.panels.title': '面板可向四个方向拆分',
	'website.terminal.panels.lead':
		'先按前缀键 <code>Ctrl+b</code>，再按一个键。点一下右边的窗口，这些按键在这里就能用；也可以用下面的按钮、点击面板、拖动分隔线。',
	'website.terminal.highlights.title': '为键盘而设计，鼠标同样顺手',
	'website.terminal.highlight.prefix.title': '一个前缀键，所有操作',
	'website.terminal.highlight.prefix.body':
		'按 <code>Ctrl+b</code>，再按空格：一个可以搜索的菜单列出每项操作和它的按键。其余的不用背。',
	'website.terminal.highlight.mouse.title': '鼠标能用',
	'website.terminal.highlight.mouse.body':
		'拖动分隔线调整面板大小，拖动选择文本，右键打开菜单，按住 <code>Ctrl</code> 点击打开链接，滚轮进入历史输出。<code>Ctrl+b</code> <code>m</code> 把鼠标交还给外层终端。',
	'website.terminal.highlight.copyMode.title': '复制模式',
	'website.terminal.highlight.copyMode.body':
		'在输出里放一个光标：用 vi 按键移动，向前或向后搜索，选择并复制，全程不碰鼠标。',
	'website.terminal.highlight.find.title': '在面板中查找',
	'website.terminal.highlight.find.body':
		'在当前面板的输出里查找。<code>Alt+C</code> 切换是否区分大小写。',
	'website.terminal.highlight.agents.title': '用键盘管理 Agent',
	'website.terminal.highlight.agents.body':
		"从列表里选一个 Agent CLI 启动。<code>Ctrl+b</code> <code>'{'</code> 和 <code>'}'</code> 跳到上一个、下一个 Agent。",
	'website.terminal.highlight.worktrees.title': 'Git 工作树即工作区',
	'website.terminal.highlight.worktrees.body':
		'创建一个工作树，并作为相邻的工作区打开，并行的工作各在各的分支上。<code>Ctrl+b</code> <code>Shift+O</code> 打开已有的检出目录。',
	'website.terminal.highlight.settings.title': '设置就在终端里',
	'website.terminal.highlight.settings.body':
		'七个分类，按 <code>/</code> 搜索，16 个主题，浅色和深色可以分别选择。改动保存在 <code>config.toml</code> 里。',
	'website.terminal.highlight.leave.title': '离开，再回来',
	'website.terminal.highlight.leave.body':
		'离开界面，终端继续运行。再次运行 <code>orchbee</code>，一切都在原处。会话恢复时，其中的 Agent 也会一并恢复。',
	'website.terminal.highlight.size.title': '任意窗口大小',
	'website.terminal.highlight.size.body':
		'100 列 × 24 行起为完整布局；不足 60 列时切换为适合窄窗口的紧凑布局。任何尺寸下 <code>Ctrl+b</code> <code>b</code> 都可以收起侧边栏。',
	'website.terminal.keys.title': '按键',
	'website.terminal.keys.lead':
		'以下每个键都跟在前缀键 <code>Ctrl+b</code> 之后。测试期间按键仍可能调整。',
	'website.terminal.keys.panels': '面板',
	'website.terminal.keys.panels.splitRight': '向右拆分',
	'website.terminal.keys.panels.splitDown': '向下拆分',
	'website.terminal.keys.panels.splitLeft': '向左拆分',
	'website.terminal.keys.panels.splitUp': '向上拆分',
	'website.terminal.keys.panels.focus': '移动焦点',
	'website.terminal.keys.panels.resize': '调整大小',
	'website.terminal.keys.panels.maximize': '最大化或还原',
	'website.terminal.keys.panels.move': '移动面板',
	'website.terminal.keys.panels.last': '回到上一个面板',
	'website.terminal.keys.panels.close': '关闭面板',
	'website.terminal.keys.tabs': '标签页与工作区',
	'website.terminal.keys.tabs.newTab': '新建标签页',
	'website.terminal.keys.tabs.nextTab': '下一个或上一个标签页',
	'website.terminal.keys.tabs.tabNumber': '按编号切换标签页',
	'website.terminal.keys.tabs.moveTab': '移动标签页',
	'website.terminal.keys.tabs.renameTab': '重命名标签页',
	'website.terminal.keys.tabs.closeTab': '关闭标签页',
	'website.terminal.keys.tabs.newWorkspace': '新建工作区',
	'website.terminal.keys.tabs.renameWorkspace': '重命名工作区',
	'website.terminal.keys.tabs.closeWorkspace': '关闭工作区',
	'website.terminal.keys.tabs.navigator': '导航器',
	'website.terminal.keys.agents': 'Agent 与工作树',
	'website.terminal.keys.agents.startAgent': '启动 Agent',
	'website.terminal.keys.agents.nextAgent': '上一个或下一个 Agent',
	'website.terminal.keys.agents.notification': '打开通知指向的位置',
	'website.terminal.keys.agents.createWorktree': '创建工作树',
	'website.terminal.keys.agents.openWorktree': '打开工作树',
	'website.terminal.keys.interface': '界面',
	'website.terminal.keys.interface.actionMenu': '操作菜单',
	'website.terminal.keys.interface.settings': '设置',
	'website.terminal.keys.interface.sidebar': '显示或收起侧边栏',
	'website.terminal.keys.interface.focusSidebar': '聚焦侧边栏',
	'website.terminal.keys.interface.find': '在面板中查找',
	'website.terminal.keys.interface.copyMode': '复制模式',
	'website.terminal.keys.interface.history': '进入历史输出',
	'website.terminal.keys.interface.mouse': '把鼠标交给外层终端',
	'website.terminal.keys.interface.sendPrefix': '把前缀键发送给终端',
	'website.terminal.keys.interface.leave': '离开界面；终端继续运行',
} as const satisfies Readonly<Record<WebsiteTerminalMessageId, string>>;
