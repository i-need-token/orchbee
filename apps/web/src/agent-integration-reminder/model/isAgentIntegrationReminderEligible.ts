import type { DesktopAgentIntegrationSnapshotItem } from '@orchbee/desktop-client-protocol';

export function isAgentIntegrationReminderEligible(
	integration: DesktopAgentIntegrationSnapshotItem,
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
