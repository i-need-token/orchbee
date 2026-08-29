import type { SshConnectionManagementMessageId } from '../en-US';

export const zhCNSshConnectionManagementMessages = {
	'ssh.connections.actions.add': '添加连接',
	'ssh.connections.actions.addHost': '添加',
	'ssh.connections.actions.adding': '正在添加…',
	'ssh.connections.actions.connect': '连接',
	'ssh.connections.actions.disconnect': '断开连接',
	'ssh.connections.actions.disconnecting': '正在断开…',
	'ssh.connections.actions.dismissFailure': '关闭错误提示',
	'ssh.connections.actions.refresh': '刷新',
	'ssh.connections.actions.retry': '重试',
	'ssh.connections.category': 'SSH 连接',
	'ssh.connections.description': '管理打开远程项目和文件夹时可复用的 SSH 主机。',
	'ssh.connections.discovery.invalidConfig': '部分 SSH 配置无法读取，仍会显示其中有效的 Host。',
	'ssh.connections.discovery.readFailed':
		'Orchbee 无法读取你的 SSH 配置，但你仍然可以手动添加连接。',
	'ssh.connections.discovery.truncated': '你的 SSH 配置较大，因此这里只显示最先发现的一部分 Host。',
	'ssh.connections.failure.busy': '请先完成当前 SSH 操作，然后再试。',
	'ssh.connections.failure.connection': 'Orchbee 无法更改此连接。请检查 SSH 访问权限后重试。',
	'ssh.connections.failure.control': '桌面控制连接已中断。请重新加载窗口后再试。',
	'ssh.connections.failure.storage': 'Orchbee 无法安全保存此连接。请重启 Orchbee 后再试。',
	'ssh.connections.failure.target': '无法解析这个 SSH 地址。请检查地址或 SSH Host 名称。',
	'ssh.connections.fields.destination': 'SSH 地址',
	'ssh.connections.fields.destinationDescription':
		'可使用 ~/.ssh/config 中的 Host 名称、user@hostname，或粘贴 ssh user@hostname。',
	'ssh.connections.fields.destinationPlaceholder': 'my-server 或 user@example.com',
	'ssh.connections.managed.description': '这些连接可供本设备上的所有远程项目共同使用。',
	'ssh.connections.managed.emptyDescription': '在上方添加地址，或从 SSH 配置中选择一个主机。',
	'ssh.connections.managed.emptyTitle': '还没有添加 SSH 连接',
	'ssh.connections.managed.projectCount':
		'{count, plural, =0 {尚未用于任何项目} other {已用于 # 个项目}}',
	'ssh.connections.managed.section': '你的连接',
	'ssh.connections.privacy.description':
		'Orchbee 只保存连接信息。连接时，密码和密钥口令会直接交给 SSH，且永远不会被保存。',
	'ssh.connections.privacy.title': '你的凭据不会被保存',
	'ssh.connections.registryUnavailable':
		'暂时无法读取已保存的 SSH 连接。仍可查看自动发现的主机，但不能添加或连接。',
	'ssh.connections.source.orchbee': '已添加到 Orchbee',
	'ssh.connections.source.system': '系统 SSH 配置',
	'ssh.connections.status.blocked': '需要处理',
	'ssh.connections.status.connected': '已连接',
	'ssh.connections.status.connecting': '正在连接…',
	'ssh.connections.status.disconnected': '未连接',
	'ssh.connections.status.interactionRequired': '需要登录',
	'ssh.connections.status.retrying': '正在重新连接…',
	'ssh.connections.status.unavailable': '不可用',
	'ssh.connections.suggestions.description':
		'Orchbee 自动发现了本设备上的这些具体 Host。添加后，就可以在远程项目中复用。',
	'ssh.connections.suggestions.emptyDescription':
		'在 ~/.ssh/config 中添加其他具体 Host，然后刷新此列表。',
	'ssh.connections.suggestions.emptyTitle': '没有发现其他 SSH 主机',
	'ssh.connections.suggestions.section': '来自你的 SSH 配置',
	'ssh.connections.title': 'SSH 连接',
} as const satisfies Readonly<Record<SshConnectionManagementMessageId, string>>;
