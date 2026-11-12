import type {
	AgentIntegrationCommandPurpose,
	AgentIntegrationId,
	AgentIntegrationSnapshotItem,
} from './agent-integration-management-control';

/**
 * The Agents read only through their own program, which may create its own files when it starts:
 * `list` never starts it and leaves such a row not checked, while `explain`, `enable`, and
 * `disable` start it. Mirrors `IntegrationId::read_by_starting_agent` in
 * `orchbee-agent-integration`.
 */
const READ_BY_STARTING_AGENT: ReadonlySet<AgentIntegrationId> = new Set([
	'claude-code',
	'github-copilot',
	'grok',
	'hermes',
]);

type ItemFacts = Pick<
	AgentIntegrationSnapshotItem,
	'availability' | 'integrationId' | 'issue' | 'managementMode' | 'state'
>;

/** Whether an `explain` of this row starts the Agent's program; mirrors `explain_starts_agent`. */
export function explainStartsAgent(item: ItemFacts): boolean {
	return (
		READ_BY_STARTING_AGENT.has(item.integrationId) &&
		item.managementMode === 'managed-local' &&
		item.availability !== 'not-found'
	);
}

/**
 * Whether a reported row was left unread: a found, managed Agent read only through its own program
 * whose state is unknown with no issue and no timeout. Mirrors `SnapshotItem::not_checked`; a
 * failed or timed-out read always carries an issue.
 */
export function reportedNotChecked(item: ItemFacts & { readonly timedOut: boolean }): boolean {
	return (
		READ_BY_STARTING_AGENT.has(item.integrationId) &&
		item.availability === 'available' &&
		item.managementMode === 'managed-local' &&
		item.state === 'unknown' &&
		item.issue === null &&
		!item.timedOut
	);
}

/** Reads a planned command's purpose from its argument vector, or null for any other command. */
export function agentIntegrationCommandPurpose(
	argv: readonly string[],
): AgentIntegrationCommandPurpose | null {
	const [family, verb, object] = argv;
	if (family !== 'plugin' && family !== 'plugins') return null;
	switch (verb) {
		case 'marketplace':
			if (object === 'add') return 'add-plugin-source';
			if (object === 'remove') return 'remove-plugin-source';
			return null;
		case 'install':
		case 'add':
			return 'install-plugin';
		case 'uninstall':
		case 'remove':
			return 'remove-plugin';
		case 'enable':
			return 'enable-plugin';
		case 'list':
			return 'confirm-plugin';
		default:
			return null;
	}
}
