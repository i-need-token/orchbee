import type { AgentIntegrationMessageId } from '../en-US';

export const zhCNAgentIntegrationMessages = {
	'agent.integrations.actions.refresh': '刷新状态',
	'agent.integrations.actions.working': '处理中…',
	'agent.integrations.category': '智能体集成',
	'agent.integrations.description': '管理可选的本地适配器，让 Orchbee 能显示更丰富的智能体状态。',
	'agent.integrations.disclosure':
		'只有你切换开关后，Orchbee 才会更改相应集成；适配器永远不会自动安装。',
	'agent.integrations.failure.agentUnavailable':
		'在本地设备上未找到此智能体。请先安装智能体，然后刷新。',
	'agent.integrations.failure.busy': '请等待当前集成操作完成。',
	'agent.integrations.failure.conflict': 'Orchbee 发现了冲突的插件或文件，并且没有改动它。',
	'agent.integrations.failure.controlUnavailable': '集成管理暂不可用。请重新打开设置后再试。',
	'agent.integrations.failure.manualActionRequired':
		'此步骤需要你在智能体中手动审阅；Orchbee 没有继续自动更改。',
	'agent.integrations.failure.operationFailed': '集成命令未能完成；下面显示的是重新检测到的状态。',
	'agent.integrations.failure.verificationFailed':
		'Orchbee 无法验证请求的状态。请检查智能体配置后刷新。',
	'agent.integrations.localSection': '本地智能体状态适配器',
	'agent.integrations.providerDescription':
		'让 Orchbee 观察 {agent} 状态，但不会读取提示词、终端输出或工具内容。',
	'agent.integrations.remoteDescription':
		'远端主机始终只能手动配置。请在智能体实际运行的主机执行经过审阅的设置命令；Orchbee 永远不会复制文件或修改远端配置。',
	'agent.integrations.remoteTitle': '远端主机保持手动配置',
	'agent.integrations.status.conflict': '存在冲突',
	'agent.integrations.status.disabled': '已关闭',
	'agent.integrations.status.enabled': '已开启',
	'agent.integrations.status.inspectionFailed': '状态不可用',
	'agent.integrations.status.manualOnly': '手动设置',
	'agent.integrations.status.notFound': '未找到智能体',
	'agent.integrations.status.notInstalled': '未开启',
	'agent.integrations.status.updateRequired': '集成需要更新',
	'agent.integrations.status.unknown': '未知',
	'agent.integrations.title': '智能体集成',
	'agent.integrations.toggle.label': '{agent} 集成',
} as const satisfies Readonly<Record<AgentIntegrationMessageId, string>>;
