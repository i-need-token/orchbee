export const enUSAgentIntegrationMessages = {
	'agent.integrations.actions.explain': 'Explain',
	'agent.integrations.actions.refresh': 'Refresh status',
	'agent.integrations.actions.working': 'Working…',
	'agent.integrations.category': 'Agent integrations',
	'agent.integrations.description':
		'Manage the optional local adapters that let Orchbee show rich Agent status.',
	'agent.integrations.consent.detail':
		'This check starts {agent} to read its plugins, and {agent} may create its own files when it starts. Orchbee changes nothing. Choose Explain to start the check.',
	'agent.integrations.consent.title': 'Check {agent}?',
	'agent.integrations.diagnosis.close': 'Close',
	'agent.integrations.diagnosis.pending': 'Checking this integration on this device…',
	'agent.integrations.diagnosis.readOnly':
		'Read-only check: nothing was changed and no Agent program was started.',
	'agent.integrations.diagnosis.started':
		'Orchbee changed nothing. This check starts {agent} when needed to read its plugins, and {agent} may create its own files.',
	'agent.integrations.diagnosis.title': '{agent} diagnosis',
	'agent.integrations.disclosure':
		'Orchbee changes an integration only when you change its switch. It never installs adapters automatically.',
	'agent.integrations.explain.label': 'Explain {agent} integration',
	'agent.integrations.fact.adapter': 'Orchbee adapter',
	'agent.integrations.fact.adapterSource': 'Adapter source',
	'agent.integrations.fact.agent': 'Agent on this device',
	'agent.integrations.fact.entry': 'Integration entry',
	'agent.integrations.fact.integration': 'Integration',
	'agent.integrations.fact.issue': 'Issue',
	'agent.integrations.fact.marketplace': 'Plugin marketplace',
	'agent.integrations.fact.plan': 'Turning on would',
	'agent.integrations.fact.plugin': 'Plugin',
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
	'agent.integrations.finding.agentMissing': 'Definitely missing: the Agent',
	'agent.integrations.finding.conflict': 'Blocked by a conflict',
	'agent.integrations.finding.disabled': 'Turned off',
	'agent.integrations.finding.manualSetup': 'Manual setup',
	'agent.integrations.finding.notInstalled': 'Definitely missing: the integration',
	'agent.integrations.finding.ready': 'Ready',
	'agent.integrations.finding.unknown': 'Result unknown',
	'agent.integrations.finding.updateRequired': 'Update needed',
	'agent.integrations.findingDetail.disabled':
		'The integration is installed but turned off. Turn it on to use it.',
	'agent.integrations.findingDetail.inspectionFailed':
		'Orchbee could not read this integration, so the state is unknown, not missing. Check the Agent configuration, then refresh.',
	'agent.integrations.findingDetail.manualSetup':
		'Orchbee does not manage this integration here; set it up manually in the Agent.',
	'agent.integrations.findingDetail.notInstalled':
		'The Agent is here, but its integration is not installed. Turn it on to install it.',
	'agent.integrations.findingDetail.ready':
		"The integration is enabled, so Orchbee can show this Agent's status.",
	'agent.integrations.findingDetail.timedOut':
		'The check ran out of time, so the state is unknown, not missing. Refresh to check again.',
	'agent.integrations.findingDetail.undetermined':
		'Orchbee could not determine the state, so it is unknown, not missing. Refresh to check again.',
	'agent.integrations.findingDetail.updateRequired':
		'The installed integration is out of date. Turn it on to update it.',
	'agent.integrations.issue.manualActionRequired': 'Needs manual review in the Agent',
	'agent.integrations.issue.unsupportedPlatform': 'Not supported on this platform',
	'agent.integrations.issue.unsupportedVersion': 'This Agent version is not supported',
	'agent.integrations.localSection': 'Local Agent status adapters',
	'agent.integrations.plan.command': 'use {command} to {purposes}',
	'agent.integrations.plan.join': '{first}; {rest}',
	'agent.integrations.plan.none': 'make no changes',
	'agent.integrations.plan.purpose.addPluginSource': "add Orchbee's plugin source",
	'agent.integrations.plan.purpose.confirmPlugin': 'confirm the plugin is on',
	'agent.integrations.plan.purpose.enablePlugin': 'turn the plugin on',
	'agent.integrations.plan.purpose.installPlugin': 'install the Orchbee plugin',
	'agent.integrations.plan.purpose.other': 'run a plugin command',
	'agent.integrations.plan.purpose.removePlugin': 'remove the old plugin',
	'agent.integrations.plan.purpose.removePluginSource': 'remove the old plugin source',
	'agent.integrations.plan.publishAdapter': 'publish the Orchbee adapter',
	'agent.integrations.plan.removeEntry': 'remove an obsolete entry',
	'agent.integrations.plan.writeEntry': 'write the integration entry',
	'agent.integrations.providerDescription':
		'Allows Orchbee to observe {agent} status without reading prompts, terminal output, or tool content.',
	'agent.integrations.remoteDescription':
		'Remote hosts are always manual-only. Run the reviewed setup command on the host where the Agent runs; Orchbee never copies files or changes remote configuration.',
	'agent.integrations.remoteTitle': 'Remote hosts stay manual',
	'agent.integrations.status.conflict': 'Conflict',
	'agent.integrations.status.disabled': 'Disabled',
	'agent.integrations.status.enabled': 'Enabled',
	'agent.integrations.status.inspectionFailed': 'Status unavailable',
	'agent.integrations.status.manualOnly': 'Manual setup',
	'agent.integrations.status.notChecked': 'Not checked',
	'agent.integrations.status.notFound': 'Agent not found',
	'agent.integrations.status.notInstalled': 'Not enabled',
	'agent.integrations.status.unknown': 'Unknown',
	'agent.integrations.status.updateRequired': 'Integration update needed',
	'agent.integrations.title': 'Agent integrations',
	'agent.integrations.toggle.label': '{agent} integration',
	'agent.integrations.value.adapterBindingFailed': 'Does not match this Orchbee version',
	'agent.integrations.value.adapterNotReady': 'Not ready',
	'agent.integrations.value.adapterReady': 'Ready',
	'agent.integrations.value.adapterSourceAbsent': 'Not included in this installation',
	'agent.integrations.value.conflict': 'Conflict',
	'agent.integrations.value.current': 'Current',
	'agent.integrations.value.found': 'Found',
	'agent.integrations.value.installed': 'Installed',
	'agent.integrations.value.missing': 'Missing',
	'agent.integrations.value.notAdded': 'Not added',
	'agent.integrations.value.notFound': 'Not found',
	'agent.integrations.value.notInstalled': 'Not installed',
	'agent.integrations.value.unknown': 'Unknown',
	'agent.integrations.value.updateNeeded': 'Update needed',
} as const;

export type AgentIntegrationMessageId = keyof typeof enUSAgentIntegrationMessages;
