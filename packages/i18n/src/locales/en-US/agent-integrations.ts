export const enUSAgentIntegrationMessages = {
	'agent.integrations.actions.disable': 'Disable',
	'agent.integrations.actions.enable': 'Enable',
	'agent.integrations.actions.refresh': 'Refresh status',
	'agent.integrations.actions.working': 'Working…',
	'agent.integrations.category': 'Agent integrations',
	'agent.integrations.description':
		'Manage the optional local adapters that let Orchbee show rich Agent status.',
	'agent.integrations.disclosure':
		'Orchbee only changes an integration after you choose Enable or Disable. It never installs adapters automatically.',
	'agent.integrations.failure.agentUnavailable':
		'This Agent was not found on the local device. Install it first, then refresh.',
	'agent.integrations.failure.busy': 'Wait for the current integration operation to finish.',
	'agent.integrations.failure.conflict':
		'Orchbee found a conflicting plugin or file and left it unchanged.',
	'agent.integrations.failure.controlUnavailable':
		'Integration management is unavailable. Reopen Settings and try again.',
	'agent.integrations.failure.manualActionRequired':
		'This step needs manual review in the Agent. No further changes were made automatically.',
	'agent.integrations.failure.operationFailed':
		'The integration command did not complete. The detected state is shown below.',
	'agent.integrations.failure.verificationFailed':
		'Orchbee could not verify the requested state. Refresh after checking the Agent configuration.',
	'agent.integrations.localSection': 'Local Agent status adapters',
	'agent.integrations.providerDescription':
		'Allows Orchbee to observe {agent} status without reading prompts, terminal output, or tool content.',
	'agent.integrations.remoteDescription':
		'Remote hosts are always manual-only. Run the reviewed setup command on the host where the Agent runs; Orchbee never copies files or changes remote configuration.',
	'agent.integrations.remoteTitle': 'Remote hosts stay manual',
	'agent.integrations.reminder.body':
		'{agent} is installed locally, but its Orchbee status integration is not enabled.',
	'agent.integrations.reminder.dismiss': 'Dismiss {agent} integration reminder',
	'agent.integrations.reminder.open': 'Review integration',
	'agent.integrations.reminder.title': 'Show richer Agent status',
	'agent.integrations.status.conflict': 'Conflict',
	'agent.integrations.status.disabled': 'Disabled',
	'agent.integrations.status.enabled': 'Enabled',
	'agent.integrations.status.inspectionFailed': 'Status unavailable',
	'agent.integrations.status.manualOnly': 'Manual setup',
	'agent.integrations.status.notFound': 'Agent not found',
	'agent.integrations.status.notInstalled': 'Not enabled',
	'agent.integrations.status.unknown': 'Unknown',
	'agent.integrations.title': 'Agent integrations',
} as const;

export type AgentIntegrationMessageId = keyof typeof enUSAgentIntegrationMessages;
