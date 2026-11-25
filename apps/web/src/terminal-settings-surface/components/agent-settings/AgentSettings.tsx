import { agentObservationMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { useUserSettings } from '../../hooks/useUserSettings';
import type { AgentLaunchCatalogSource } from '../../types/terminal-settings-surface';
import { SettingsGroup } from '../settings-group/SettingsGroup';

import styles from './AgentSettings.module.css';
import { AgentConfirmationRow } from './components/agent-confirmation-row/AgentConfirmationRow';
import { AgentLaunchSettings } from './components/agent-launch-settings/AgentLaunchSettings';
import { QuotaResumeSettings } from './components/quota-resume-settings/QuotaResumeSettings';

interface AgentSettingsProps {
	readonly catalog?: AgentLaunchCatalogSource | undefined;
	readonly query: string;
	/** Opens the page where more Agents are requested; absent where no external browser is wired. */
	readonly requestAgent?: (() => void) | undefined;
	readonly userSettings: UserSettingsControl;
}

/** The Agent types Orchbee can answer routine confirmations for, each with its saved switch. */
const CONFIRMATIONS = [
	{
		label: agentObservationMessages.providerClaudeCode,
		providerId: 'claude-code',
		settingId: 'agents.auto-allow.claude',
		settingKey: 'agents.auto_allow.claude',
	},
	{
		label: agentObservationMessages.providerCodex,
		providerId: 'codex',
		settingId: 'agents.auto-allow.codex',
		settingKey: 'agents.auto_allow.codex',
	},
	{
		label: agentObservationMessages.providerGeminiCli,
		providerId: 'gemini',
		settingId: 'agents.auto-allow.gemini',
		settingKey: 'agents.auto_allow.gemini',
	},
	{
		label: agentObservationMessages.providerOpenCode,
		providerId: 'opencode',
		settingId: 'agents.auto-allow.opencode',
		settingKey: 'agents.auto_allow.opencode',
	},
] as const;

/**
 * Agent confirmation, quota resume and launch settings. A switch shows only while the Server
 * publishes its key, as an older Session's Server does not, and the group only with one.
 */
export function AgentSettings({
	catalog,
	query,
	requestAgent,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { snapshot } = useUserSettings(userSettings);
	const confirmations = CONFIRMATIONS.filter(
		(confirmation) => snapshot !== null && Object.hasOwn(snapshot.values, confirmation.settingKey),
	);
	const request = intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsRequest);
	return (
		<>
			{confirmations.length === 0 ? null : (
				<SettingsGroup
					description={intl.formatMessage(
						terminalSettingsMessages.pageAgentConfirmationsDescription,
					)}
					label={intl.formatMessage(terminalSettingsMessages.pageGroupAgentConfirmations)}
					note={
						requestAgent === undefined ? (
							request
						) : (
							<span className={styles['request']}>
								<Button
									data-agent-request
									size='xs'
									type='button'
									variant='link'
									onClick={requestAgent}
								>
									{request}
								</Button>
							</span>
						)
					}
				>
					{confirmations.map((confirmation) => (
						<AgentConfirmationRow
							key={confirmation.settingKey}
							label={intl.formatMessage(confirmation.label)}
							query={query}
							providerId={confirmation.providerId}
							settingId={confirmation.settingId}
							settingKey={confirmation.settingKey}
							userSettings={userSettings}
						/>
					))}
				</SettingsGroup>
			)}
			<QuotaResumeSettings query={query} userSettings={userSettings} />
			{/* Last, since it lists every Agent and would push the switches above out of view. */}
			<AgentLaunchSettings catalog={catalog} query={query} userSettings={userSettings} />
		</>
	);
}
