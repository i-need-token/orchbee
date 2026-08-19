import type { TerminalSettingsMessageId } from '../en-US';

export const zhCNTerminalSettingsMessages = {
	'terminal.settings.actions.back': '返回',
	'terminal.settings.actions.reset': '重置',
	'terminal.settings.applicationTheme.dark': '深色',
	'terminal.settings.applicationTheme.description': '选择 Orchbee 的浅色或深色外观。',
	'terminal.settings.applicationTheme.label': '应用主题',
	'terminal.settings.applicationTheme.light': '浅色',
	'terminal.settings.browserAccess.actions.open': '打开浏览器访问',
	'terminal.settings.browserAccess.description': '通过浏览器使用此桌面端终端。',
	'terminal.settings.browserAccess.title': '浏览器访问',
	'terminal.settings.browserAccess.unavailable': '此应用中无法使用浏览器访问。',
	'terminal.settings.categories.browserAccess': '浏览器访问',
	'terminal.settings.categories.ghosttyMigration': 'Ghostty 迁移',
	'terminal.settings.categories.macosInput': 'macOS 输入',
	'terminal.settings.categories.remoteDevelopment': '远程开发',
	'terminal.settings.categories.shellIntegration': 'Shell 集成',
	'terminal.settings.categories.terminal': '终端',
	'terminal.settings.colors.background.description': '选择终端文本后方的默认颜色。',
	'terminal.settings.colors.background.invalid': '请输入 #RRGGBB 格式的颜色。',
	'terminal.settings.colors.background.label': '背景色',
	'terminal.settings.colors.backgroundOpacity.description': '选择终端背景的显示不透明度。',
	'terminal.settings.colors.backgroundOpacity.invalid':
		'请输入 {minimum, number} 到 {maximum, number} 之间的整数。',
	'terminal.settings.colors.backgroundOpacity.label': '背景不透明度',
	'terminal.settings.colors.backgroundOpacity.percentValue': '{value, number}%',
	'terminal.settings.colors.displayTheme.currentDeviceOnlyNotice':
		'非主机主题只会更改此设备上的终端颜色显示。',
	'terminal.settings.colors.displayTheme.custom': '自定义',
	'terminal.settings.colors.displayTheme.dark': '深色',
	'terminal.settings.colors.displayTheme.description': '选择终端前景色和背景色的显示方式。',
	'terminal.settings.colors.displayTheme.host': '主机',
	'terminal.settings.colors.displayTheme.label': '终端显示主题',
	'terminal.settings.colors.displayTheme.light': '浅色',
	'terminal.settings.colors.foreground.description': '选择终端文本的默认颜色。',
	'terminal.settings.colors.foreground.invalid': '请输入 #RRGGBB 格式的颜色。',
	'terminal.settings.colors.foreground.label': '前景色',
	'terminal.settings.description': '自定义应用和终端的外观。',
	'terminal.settings.macosInput.description': '选择这台 Mac 如何把键盘输入发送给终端程序。',
	'terminal.settings.macosInput.optionAsAlt.both': '左右 Option 键',
	'terminal.settings.macosInput.optionAsAlt.description':
		'选中的 Option 键会发送终端 Alt 组合键，而不是输入 macOS 特殊字符；已打开的终端会立即更新。',
	'terminal.settings.macosInput.optionAsAlt.label': '将 Option 用作 Alt',
	'terminal.settings.macosInput.optionAsAlt.left': '仅左 Option 键',
	'terminal.settings.macosInput.optionAsAlt.off': '关闭 — 输入 macOS 特殊字符',
	'terminal.settings.macosInput.optionAsAlt.right': '仅右 Option 键',
	'terminal.settings.macosInput.sections.optionKey': 'Option 键',
	'terminal.settings.macosInput.title': 'macOS 输入',
	'terminal.settings.ghosttyMigration.actions.analyze': '分析配置',
	'terminal.settings.ghosttyMigration.actions.clear': '清除',
	'terminal.settings.ghosttyMigration.description':
		'查看常用 Ghostty 设置中哪些可直接带入、哪些需手工处理、哪些不会迁移。',
	'terminal.settings.ghosttyMigration.dispositions.ignored': '不会迁移',
	'terminal.settings.ghosttyMigration.dispositions.invalid': '需要修正',
	'terminal.settings.ghosttyMigration.dispositions.manual': '手工处理',
	'terminal.settings.ghosttyMigration.dispositions.supported': '可直接带入',
	'terminal.settings.ghosttyMigration.empty': '粘贴配置文本后即可生成迁移报告。',
	'terminal.settings.ghosttyMigration.lineNumber': '第 {line, number} 行',
	'terminal.settings.ghosttyMigration.noAssignments': '未找到配置赋值。',
	'terminal.settings.ghosttyMigration.reasons.cellOpacity':
		'Ghostty 可让显式单元格背景透明；Orchbee 当前设置只改变默认背景。',
	'terminal.settings.ghosttyMigration.reasons.color':
		'Ghostty 还接受 X11 命名颜色，请手工选择对应的 #RRGGBB 值。',
	'terminal.settings.ghosttyMigration.reasons.configFile':
		'没有读取被包含的文件，请另行分析其内容。',
	'terminal.settings.ghosttyMigration.reasons.customColorTarget':
		'使用此值前请先选择“自定义”终端显示主题；另一项自定义颜色会保留 Orchbee 当前值。',
	'terminal.settings.ghosttyMigration.reasons.fontDetails': 'Orchbee 当前设置中没有这项字体细节。',
	'terminal.settings.ghosttyMigration.reasons.fontFallback':
		'Ghostty 会保留有顺序的回退字体列表，而 Orchbee 当前设置只接受一个值。',
	'terminal.settings.ghosttyMigration.reasons.fontFamily':
		'此字体族无法安全表示为一个当前 Orchbee 设置值。',
	'terminal.settings.ghosttyMigration.reasons.fontSize':
		'Orchbee 目前只支持 8 到 48 之间的整数终端字号。',
	'terminal.settings.ghosttyMigration.reasons.invalidValue': '迁移分析器无法解释这个值。',
	'terminal.settings.ghosttyMigration.reasons.keybind':
		'不会导入 Ghostty Action 名称和按键语法，请在键盘快捷键中重新设置。',
	'terminal.settings.ghosttyMigration.reasons.losslessTarget':
		'此值在当前 Orchbee 设置中有无损对应项。',
	'terminal.settings.ghosttyMigration.reasons.malformedLine':
		'迁移报告只接受 key = value 格式的行。',
	'terminal.settings.ghosttyMigration.reasons.notInWhitelist':
		'此键不在迁移白名单中，可能是 Ghostty 专属选项或拼写错误。',
	'terminal.settings.ghosttyMigration.reasons.opacity':
		'Orchbee 只支持 10% 到 100% 的整数背景不透明度；此值需要取整或限制范围。',
	'terminal.settings.ghosttyMigration.reasons.reset':
		'此项会把 Ghostty 恢复为默认值，因此没有自定义值需要带入。',
	'terminal.settings.ghosttyMigration.reasons.superseded': '此键后面的有效赋值优先生效。',
	'terminal.settings.ghosttyMigration.reasons.theme':
		'Ghostty 主题可能加载文件和多项设置，请手工选择 Orchbee 终端主题。',
	'terminal.settings.ghosttyMigration.reasons.windowTheme':
		'Ghostty 窗口主题与 Orchbee 应用主题的作用范围不同。',
	'terminal.settings.ghosttyMigration.reasons.workingDirectory':
		'请在 Orchbee 中选择项目或工作区文件夹，不要直接导入此值。',
	'terminal.settings.ghosttyMigration.rejected.lineTooLong':
		'第 {line, number} 行超过 {limit, number} 个字符的限制，未分析任何内容。',
	'terminal.settings.ghosttyMigration.rejected.sourceTooLarge':
		'文本超过 {limit, number} 个字符的限制，未分析任何内容。',
	'terminal.settings.ghosttyMigration.rejected.tooManyLines':
		'文本超过 {limit, number} 行的限制，未分析任何内容。',
	'terminal.settings.ghosttyMigration.reportTitle': '迁移报告',
	'terminal.settings.ghosttyMigration.safetyNotice': '这只是预检，Orchbee 不会更改或保存任何设置。',
	'terminal.settings.ghosttyMigration.source.description':
		'粘贴 Ghostty 配置文件的内容；被包含的文件和主题不会被打开。',
	'terminal.settings.ghosttyMigration.source.label': 'Ghostty 配置',
	'terminal.settings.ghosttyMigration.source.placeholder': 'font-family = JetBrains Mono',
	'terminal.settings.ghosttyMigration.target': 'Orchbee 设置',
	'terminal.settings.ghosttyMigration.title': '从 Ghostty 迁移',
	'terminal.settings.persistence.invalidWarning':
		'已保存的设置无效，当前正在使用默认设置；重置可尝试保存默认设置。',
	'terminal.settings.persistence.readWarning':
		'无法读取已保存的设置，当前正在使用默认设置；重置可尝试保存默认设置。',
	'terminal.settings.persistence.writeWarning': '更改已应用于当前会话，但未能保存。',
	'terminal.settings.profiles.actions.add': '添加 Profile',
	'terminal.settings.profiles.actions.cancel': '取消',
	'terminal.settings.profiles.actions.confirmDelete': '确认删除此 Profile',
	'terminal.settings.profiles.actions.confirmReset': '确认重置全部 Profile',
	'terminal.settings.profiles.actions.delete': '删除',
	'terminal.settings.profiles.actions.duplicate': '复制',
	'terminal.settings.profiles.actions.makeDefault': '设为默认',
	'terminal.settings.profiles.actions.reset': '重置 Profile',
	'terminal.settings.profiles.actions.save': '保存更改',
	'terminal.settings.profiles.addName.label': '新 Profile 名称',
	'terminal.settings.profiles.addName.placeholder': 'Profile 名称',
	'terminal.settings.profiles.arguments.actions.add': '添加参数',
	'terminal.settings.profiles.arguments.actions.remove': '移除参数 {number, number}',
	'terminal.settings.profiles.arguments.argumentLabel': '参数 {number, number}',
	'terminal.settings.profiles.arguments.description':
		'参数会按此顺序原样传入，不会解析 Shell 引号。',
	'terminal.settings.profiles.arguments.label': '参数',
	'terminal.settings.profiles.command.custom': '自定义可执行文件',
	'terminal.settings.profiles.command.label': '命令',
	'terminal.settings.profiles.command.system': '系统 Shell',
	'terminal.settings.profiles.defaultBadge': '默认',
	'terminal.settings.profiles.executable.description': '请输入此设备上可执行文件的绝对路径。',
	'terminal.settings.profiles.executable.label': '可执行文件路径',
	'terminal.settings.profiles.executable.placeholder': '/usr/local/bin/fish',
	'terminal.settings.profiles.failures.busy': '请等待当前 Profile 更改完成。',
	'terminal.settings.profiles.failures.catalogFull': '已达到 Profile 数量上限。',
	'terminal.settings.profiles.failures.controlUnavailable':
		'Profile 控制暂不可用，请重新打开设置后重试。',
	'terminal.settings.profiles.failures.invalid': '请检查 Profile 名称、可执行文件路径和参数。',
	'terminal.settings.profiles.failures.notFound': '此 Profile 已不存在。',
	'terminal.settings.profiles.failures.protectedDelete':
		'请先选择另一个默认 Profile，再删除此 Profile。',
	'terminal.settings.profiles.failures.unsupported': '此启动选项不受当前操作系统支持。',
	'terminal.settings.profiles.failures.write': '更改未能保存；之前的 Profile 仍然有效。',
	'terminal.settings.profiles.futureTerminalsNotice':
		'新终端启动时会记住当时的默认 Profile；正在运行和恢复的终端会保留原来的命令。',
	'terminal.settings.profiles.login.description':
		'通过传入 -l，把系统 Shell 作为 POSIX 登录 Shell 启动。',
	'terminal.settings.profiles.login.label': '登录 Shell',
	'terminal.settings.profiles.login.unsupported': '当前操作系统不支持登录 Shell 模式。',
	'terminal.settings.profiles.name.invalid': '请输入 Profile 名称。',
	'terminal.settings.profiles.name.label': 'Profile 名称',
	'terminal.settings.profiles.persistence.invalid':
		'已保存的 Profile 文件无效；保存或重置前将使用内置 Profile。',
	'terminal.settings.profiles.persistence.read':
		'无法读取已保存的 Profile 文件；保存或重置前将使用内置 Profile。',
	'terminal.settings.profiles.persistence.write':
		'上一次 Profile 更改未能保存；之前的 Profile 仍然有效。',
	'terminal.settings.profiles.section': '本地 Shell Profile',
	'terminal.settings.profiles.systemShell': '系统 Shell',
	'terminal.settings.reset.description': '恢复默认的应用和终端设置。',
	'terminal.settings.remoteDevelopment.baseline.description':
		'Orchbee 会用这个广泛可用的 TERM 启动本地桌面终端；标准 SSH PTY 可将它传给远端 shell。',
	'terminal.settings.remoteDevelopment.baseline.title': '终端基线',
	'terminal.settings.remoteDevelopment.description':
		'在不修改远端主机的前提下，检查远端 shell 实际看到的终端标识。',
	'terminal.settings.remoteDevelopment.diagnostic.description':
		'通过 SSH 连接后，请自行运行此命令。它只读取三个环境值，并检查 infocmp 能否识别 TERM。',
	'terminal.settings.remoteDevelopment.diagnostic.label': '只读诊断命令',
	'terminal.settings.remoteDevelopment.diagnostic.resultDescription':
		'TERM_MATCH 用来比较 TERM 与 Orchbee 基线；含不安全控制字符的值不会被直接打印。TERMINFO 会区分 TERM 未设置或不安全、没有 infocmp、条目可用和条目缺失。',
	'terminal.settings.remoteDevelopment.diagnostic.title': '诊断远端 shell',
	'terminal.settings.remoteDevelopment.noChanges.description':
		'Orchbee 不会包装 SSH、静默转发额外环境变量、安装 terminfo、写入远端文件或保留远端缓存；你的 OpenSSH 配置仍然是最终依据。',
	'terminal.settings.remoteDevelopment.noChanges.title': '不会自动修改远端',
	'terminal.settings.remoteDevelopment.title': '远程开发',
	'terminal.settings.shellIntegration.bash.title': 'Bash',
	'terminal.settings.shellIntegration.bash.version':
		'需要 Bash 4.4 或更高版本；macOS 自带的 Bash 3.2 不受支持。',
	'terminal.settings.shellIntegration.description': '为命令完成提醒和终端语义提供可靠的命令边界。',
	'terminal.settings.shellIntegration.fish.title': 'Fish',
	'terminal.settings.shellIntegration.manualNotice':
		'Orchbee 永远不会编辑 Shell 启动文件。请自行添加一行 source；删除或注释该行即可停用集成。',
	'terminal.settings.shellIntegration.notification.description':
		'只有点击这里的按钮才会请求系统通知权限；终端输出永远不会触发权限请求。',
	'terminal.settings.shellIntegration.notification.request': '允许系统通知',
	'terminal.settings.shellIntegration.notification.status.denied':
		'权限已拒绝，请到浏览器或操作系统设置中修改通知权限。',
	'terminal.settings.shellIntegration.notification.status.granted': '已允许系统通知。',
	'terminal.settings.shellIntegration.notification.status.prompt':
		'尚未请求；在你点击下面的按钮前，Orchbee 不会询问。',
	'terminal.settings.shellIntegration.notification.status.unsupported':
		'当前客户端不支持系统通知。',
	'terminal.settings.shellIntegration.notification.title': '系统通知',
	'terminal.settings.shellIntegration.reminder.always': '始终提醒',
	'terminal.settings.shellIntegration.reminder.description':
		'只对已启用集成发出的语义命令完成事件提醒，提醒中永远不包含命令文本。',
	'terminal.settings.shellIntegration.reminder.minimum.description':
		'运行时间短于此秒数的命令不会产生提醒。',
	'terminal.settings.shellIntegration.reminder.minimum.invalid':
		'请输入 {minimum, number} 到 {maximum, number} 之间的整数。',
	'terminal.settings.shellIntegration.reminder.minimum.label': '最短命令时长',
	'terminal.settings.shellIntegration.reminder.mode.label': '命令完成提醒',
	'terminal.settings.shellIntegration.reminder.never': '从不提醒',
	'terminal.settings.shellIntegration.reminder.title': '命令完成提醒',
	'terminal.settings.shellIntegration.reminder.unfocused': '当前终端失去焦点时',
	'terminal.settings.shellIntegration.reset.description':
		'关闭命令完成提醒，并把最短时长恢复为五秒。',
	'terminal.settings.shellIntegration.setupCommand.label': '手动 source 命令',
	'terminal.settings.shellIntegration.title': 'Shell 集成',
	'terminal.settings.shellIntegration.zsh.title': 'Zsh',
	'terminal.settings.sections.applicationAppearance': '应用外观',
	'terminal.settings.sections.terminalColors': '终端颜色',
	'terminal.settings.sections.terminalTypography': '终端字体',
	'terminal.settings.title': '设置',
	'terminal.settings.typography.fontFamily.description': '设置终端文本使用的字体。',
	'terminal.settings.typography.fontFamily.deviceAvailability':
		'字体是否可用取决于当前设备；需要时会使用回退字体。',
	'terminal.settings.typography.fontFamily.invalid': '请输入 1 到 256 个字符的字体族。',
	'terminal.settings.typography.fontFamily.label': '终端字体族',
	'terminal.settings.typography.fontSize.description': '选择终端文本的字号。',
	'terminal.settings.typography.fontSize.invalid':
		'请输入 {minimum, number} 到 {maximum, number} 之间的整数。',
	'terminal.settings.typography.fontSize.label': '终端字号',
	'terminal.settings.search.noResults': '未找到设置。',
	'terminal.settings.search.placeholder': '搜索设置',
} as const satisfies Readonly<Record<TerminalSettingsMessageId, string>>;
