import { agentObservationMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
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

/** Agent confirmation, quota resume and launch settings. */
export function AgentSettings({
	catalog,
	query,
	requestAgent,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const request = intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsRequest);
	return (
		<>
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
					providerId='claude-code'
					settingId='agents.auto-allow.claude'
					settingKey='agents.auto_allow.claude'
					userSettings={userSettings}
				/>
				<AgentConfirmationRow
					label={intl.formatMessage(agentObservationMessages.providerCodex)}
					query={query}
					providerId='codex'
					settingId='agents.auto-allow.codex'
					settingKey='agents.auto_allow.codex'
					userSettings={userSettings}
				/>
				<AgentConfirmationRow
					label={intl.formatMessage(agentObservationMessages.providerGeminiCli)}
					query={query}
					providerId='gemini'
					settingId='agents.auto-allow.gemini'
					settingKey='agents.auto_allow.gemini'
					userSettings={userSettings}
				/>
				<AgentConfirmationRow
					label={intl.formatMessage(agentObservationMessages.providerOpenCode)}
					query={query}
					providerId='opencode'
					settingId='agents.auto-allow.opencode'
					settingKey='agents.auto_allow.opencode'
					userSettings={userSettings}
				/>
			</SettingsGroup>
			<QuotaResumeSettings query={query} userSettings={userSettings} />
			{/* Last, since it lists every Agent and would push the switches above out of view. */}
			<AgentLaunchSettings catalog={catalog} query={query} userSettings={userSettings} />
		</>
	);
}
