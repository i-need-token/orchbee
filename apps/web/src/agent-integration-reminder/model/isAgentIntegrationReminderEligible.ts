import type { AgentIntegrationSnapshotItem } from '../../agent-integration-management';

export function isAgentIntegrationReminderEligible(
	integration: AgentIntegrationSnapshotItem,
): boolean {
	return (
		integration.availability === 'available' &&
		integration.issue === null &&
		integration.managementMode === 'managed-local' &&
		(integration.state === 'disabled' ||
			integration.state === 'not-installed' ||
			integration.state === 'update-required')
	);
}
