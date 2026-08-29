import type { DesktopSshProjectMessageId } from '../en-US';

export const zhCNDesktopSshProjectMessages = {
	'desktop.sshProject.actions.cancel': '取消',
	'desktop.sshProject.actions.close': '关闭',
	'desktop.sshProject.actions.connect': '连接',
	'desktop.sshProject.actions.continue': '继续',
	'desktop.sshProject.actions.retry': '重试',
	'desktop.sshProject.connection.description':
		'Orchbee 将重新连接此项目，不会更改已保存的结构或远端文件。',
	'desktop.sshProject.connection.title': '连接远端项目',
	'desktop.sshProject.description':
		'连接到可信的 SSH 主机，验证远端项目文件夹，然后将其添加到 Orchbee。',
	'desktop.sshProject.failure.busy': '另一个 SSH 项目操作正在进行。',
	'desktop.sshProject.failure.connection':
		'Orchbee 无法建立远端运行时连接。请检查 SSH 访问权限后重试。',
	'desktop.sshProject.failure.control': '桌面控制连接已中断。请重新加载窗口后再试。',
	'desktop.sshProject.failure.projectPath':
		'无法使用远端项目路径。请确认它是可访问的绝对文件夹路径。',
	'desktop.sshProject.failure.storage': 'Orchbee 无法安全记录此 SSH 环境。请重启 Orchbee 后再试。',
	'desktop.sshProject.failure.target': '无法安全解析 SSH 目标或配置。',
	'desktop.sshProject.failure.title': '无法连接远端项目',
	'desktop.sshProject.failure.unsupported': '此桌面平台不支持 SSH 项目连接。',
	'desktop.sshProject.failure.workbench':
		'Orchbee 无法安全地将项目添加到共享工作台。请重新加载后再试。',
	'desktop.sshProject.fields.configurationFilePath': 'SSH 配置文件（可选）',
	'desktop.sshProject.fields.configurationFilePathDescription':
		'请输入 OpenSSH 配置文件的绝对路径。',
	'desktop.sshProject.fields.configurationFilePathPlaceholder': '/Users/you/.ssh/config',
	'desktop.sshProject.fields.destination': 'SSH 目标',
	'desktop.sshProject.fields.destinationPlaceholder': 'build-host 或 user@example.com',
	'desktop.sshProject.fields.name': '环境名称',
	'desktop.sshProject.fields.namePlaceholder': '构建机器',
	'desktop.sshProject.fields.projectPath': '远端项目路径',
	'desktop.sshProject.fields.projectPathDescription': '请输入远端主机上的绝对 POSIX 文件夹路径。',
	'desktop.sshProject.fields.projectPathPlaceholder': '/srv/project',
	'desktop.sshProject.progress.committingProject': '正在将项目添加到工作台…',
	'desktop.sshProject.progress.connecting': '正在连接并准备远端运行时…',
	'desktop.sshProject.progress.inspectingProject': '正在验证远端项目文件夹…',
	'desktop.sshProject.progress.recordingEnvironment': '正在记录 SSH 环境…',
	'desktop.sshProject.progress.resolvingTarget': '正在解析 SSH 配置…',
	'desktop.sshProject.prompt.authenticationDescription':
		'此响应会直接发送给当前 SSH 进程，不会被保存。',
	'desktop.sshProject.prompt.authenticationTitle': '需要 SSH 认证',
	'desktop.sshProject.prompt.hostDescription': '继续前请检查 SSH 主机验证提示。',
	'desktop.sshProject.prompt.hostTitle': '验证 SSH 主机',
	'desktop.sshProject.prompt.response': '响应',
	'desktop.sshProject.title': '通过 SSH 连接',
} as const satisfies Record<DesktopSshProjectMessageId, string>;
