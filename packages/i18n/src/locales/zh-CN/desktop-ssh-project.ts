import type { DesktopSshProjectMessageId } from '../en-US';

export const zhCNDesktopSshProjectMessages = {
	'desktop.sshProject.actions.authenticate': '认证',
	'desktop.sshProject.actions.cancel': '取消',
	'desktop.sshProject.actions.close': '关闭',
	'desktop.sshProject.actions.connect': '连接',
	'desktop.sshProject.actions.retry': '重试',
	'desktop.sshProject.actions.trustHost': '信任并继续',
	'desktop.sshProject.attachment.description':
		'输入 SSH 地址。Orchbee 会先连接，再让你为当前项目选择远程文件夹。',
	'desktop.sshProject.attachment.directoryDescription': '选择要添加到当前项目的远程文件夹。',
	'desktop.sshProject.attachment.directoryTitle': '选择远程文件夹',
	'desktop.sshProject.attachment.title': '添加 SSH 文件夹',
	'desktop.sshProject.connection.description':
		'Orchbee 将重新连接此项目，不会更改已保存的结构或远端文件。',
	'desktop.sshProject.connection.title': '连接远端项目',
	'desktop.sshProject.description': '连接 SSH 主机，然后选择要作为项目使用的远端文件夹。',
	'desktop.sshProject.directory.description': '选择要作为项目添加的远端文件夹。',
	'desktop.sshProject.directory.title': '选择项目文件夹',
	'desktop.sshProject.environmentConnection.description':
		'验证这个可复用连接。密码和密钥口令只会发送给当前 SSH 进程。',
	'desktop.sshProject.environmentConnection.failureTitle': '无法建立 SSH 连接',
	'desktop.sshProject.environmentConnection.title': '连接 SSH',
	'desktop.sshProject.failure.busy': '另一个 SSH 项目操作正在进行。',
	'desktop.sshProject.failure.connection':
		'Orchbee 无法建立远端运行时连接。请检查 SSH 访问权限后重试。',
	'desktop.sshProject.failure.control': '桌面控制连接已中断。请重新加载窗口后再试。',
	'desktop.sshProject.failure.projectAttachment':
		'此项目已使用另一个 SSH 主机，或此远程文件夹已属于另一个项目。',
	'desktop.sshProject.failure.projectPath':
		'无法使用远端项目路径。请确认它是可访问的绝对文件夹路径。',
	'desktop.sshProject.failure.storage': 'Orchbee 无法安全记录此 SSH 环境。请重启 Orchbee 后再试。',
	'desktop.sshProject.failure.target': '无法安全解析 SSH 地址。',
	'desktop.sshProject.failure.title': '无法连接远端项目',
	'desktop.sshProject.failure.unsupported': '此桌面平台不支持 SSH 项目连接。',
	'desktop.sshProject.failure.workbench':
		'Orchbee 无法安全地将项目添加到共享工作台。请重新加载后再试。',
	'desktop.sshProject.fields.destination': 'SSH 地址',
	'desktop.sshProject.fields.destinationDescription':
		'使用 SSH 配置中的主机，或输入 user@hostname。',
	'desktop.sshProject.fields.destinationError': '请输入有效的 SSH 主机，例如 user@hostname。',
	'desktop.sshProject.fields.destinationPlaceholder': 'user@hostname',
	'desktop.sshProject.progress.committingProject': '正在将项目添加到工作台…',
	'desktop.sshProject.progress.committingProjectFolder': '正在将远程文件夹添加到项目…',
	'desktop.sshProject.progress.connecting': '正在连接并准备远端运行时…',
	'desktop.sshProject.progress.inspectingProject': '正在验证远端项目文件夹…',
	'desktop.sshProject.progress.recordingEnvironment': '正在记录 SSH 环境…',
	'desktop.sshProject.progress.resolvingTarget': '正在解析 SSH 配置…',
	'desktop.sshProject.progress.selectingDirectory': '正在加载远端主目录…',
	'desktop.sshProject.prompt.authenticationDescription':
		'密码或密钥口令只会发送给当前 SSH 进程，绝不会被保存。',
	'desktop.sshProject.prompt.authenticationTitle': '需要 SSH 认证',
	'desktop.sshProject.prompt.hostDescription': '信任此连接前，请确认主机指纹。',
	'desktop.sshProject.prompt.hostTitle': '验证 SSH 主机',
	'desktop.sshProject.prompt.secret': '密码或密钥口令',
	'desktop.sshProject.title': '通过 SSH 连接',
} as const satisfies Record<DesktopSshProjectMessageId, string>;
