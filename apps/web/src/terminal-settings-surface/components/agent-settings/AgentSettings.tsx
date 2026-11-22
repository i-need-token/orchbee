import { agentObservationMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { SettingsGroup } from '../settings-group/SettingsGroup';

import styles from './AgentSettings.module.css';
import { AgentConfirmationRow } from './components/agent-confirmation-row/AgentConfirmationRow';

interface AgentSettingsProps {
	readonly query: string;
	/** Opens the page where more Agents are requested; absent where no external browser is wired. */
	readonly requestAgent?: (() => void) | undefined;
	readonly userSettings: UserSettingsControl;
}

/** For which Agent types Orchbee answers routine confirmations: one switch for each type. */
export function AgentSettings({
	query,
	requestAgent,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const request = intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsRequest);
	return (
		<SettingsGroup
			description={intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsDescription)}
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
			<AgentConfirmationRow
				label={intl.formatMessage(agentObservationMessages.providerClaudeCode)}
				query={query}
				settingId='agents.auto-allow.claude'
				settingKey='agents.auto_allow.claude'
				userSettings={userSettings}
			/>
			<AgentConfirmationRow
				label={intl.formatMessage(agentObservationMessages.providerCodex)}
				query={query}
				settingId='agents.auto-allow.codex'
				settingKey='agents.auto_allow.codex'
				userSettings={userSettings}
			/>
			<AgentConfirmationRow
				label={intl.formatMessage(agentObservationMessages.providerGeminiCli)}
				query={query}
				settingId='agents.auto-allow.gemini'
				settingKey='agents.auto_allow.gemini'
				userSettings={userSettings}
			/>
			<AgentConfirmationRow
				label={intl.formatMessage(agentObservationMessages.providerOpenCode)}
				query={query}
				settingId='agents.auto-allow.opencode'
				settingKey='agents.auto_allow.opencode'
				userSettings={userSettings}
			/>
		</SettingsGroup>
	);
}
