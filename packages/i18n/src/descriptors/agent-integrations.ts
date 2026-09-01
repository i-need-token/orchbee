import { defineMessages } from 'react-intl';

import { enUSAgentIntegrationMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSAgentIntegrationMessages);

export const agentIntegrationMessages = defineMessages({
	actionRefresh: message('agent.integrations.actions.refresh'),
	actionWorking: message('agent.integrations.actions.working'),
	category: message('agent.integrations.category'),
	description: message('agent.integrations.description'),
	disclosure: message('agent.integrations.disclosure'),
	failureAgentUnavailable: message('agent.integrations.failure.agentUnavailable'),
	failureBusy: message('agent.integrations.failure.busy'),
	failureConflict: message('agent.integrations.failure.conflict'),
	failureControlUnavailable: message('agent.integrations.failure.controlUnavailable'),
	failureManualActionRequired: message('agent.integrations.failure.manualActionRequired'),
	failureOperationFailed: message('agent.integrations.failure.operationFailed'),
	failureVerificationFailed: message('agent.integrations.failure.verificationFailed'),
	localSection: message('agent.integrations.localSection'),
	providerDescription: message(
		'agent.integrations.providerDescription',
		'Privacy-safe description for one named Agent integration. agent is a curated product name.',
	),
	remoteDescription: message('agent.integrations.remoteDescription'),
	remoteTitle: message('agent.integrations.remoteTitle'),
	reminderBody: message(
		'agent.integrations.reminder.body',
		'In-product integration reminder. agent is a curated product name.',
	),
	reminderDismiss: message(
		'agent.integrations.reminder.dismiss',
		'Accessible dismissal label. agent is a curated product name.',
	),
	reminderEnable: message('agent.integrations.reminder.enable'),
	reminderTitle: message('agent.integrations.reminder.title'),
	statusConflict: message('agent.integrations.status.conflict'),
	statusDisabled: message('agent.integrations.status.disabled'),
	statusEnabled: message('agent.integrations.status.enabled'),
	statusInspectionFailed: message('agent.integrations.status.inspectionFailed'),
	statusManualOnly: message('agent.integrations.status.manualOnly'),
	statusNotFound: message('agent.integrations.status.notFound'),
	statusNotInstalled: message('agent.integrations.status.notInstalled'),
	statusUnknown: message('agent.integrations.status.unknown'),
	title: message('agent.integrations.title'),
	toggleLabel: message(
		'agent.integrations.toggle.label',
		'Stable accessible label for one integration switch. agent is a curated product name.',
	),
});
