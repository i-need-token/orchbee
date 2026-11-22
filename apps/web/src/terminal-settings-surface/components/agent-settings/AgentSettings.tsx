import { agentObservationMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import { isSettingOffered, type UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { useUserSettings } from '../../hooks/useUserSettings';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

import styles from './AgentSettings.module.css';
import { AgentConfirmationRow } from './components/agent-confirmation-row/AgentConfirmationRow';

interface AgentSettingsProps {
	readonly query: string;
	/** Opens the page where more Agents are requested; absent where no external browser is wired. */
	readonly requestAgent?: (() => void) | undefined;
	readonly userSettings: UserSettingsControl;
}

/**
 * Whether Agent conversations resume after a restart, and for which Agent types Orchbee answers
 * routine confirmations: one switch for each type it can answer for. A Server whose build withholds
 * automatic confirmation publishes none of its keys, and the group is left out.
 */
export function AgentSettings({
	query,
	requestAgent,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const resume = useSetting(userSettings, 'agents.resume_on_restore');
	const { snapshot } = useUserSettings(userSettings);
	const resumeLabel = intl.formatMessage(terminalSettingsMessages.pageResumeAgentsLabel);
	const request = intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsRequest);
	const confirmations = isSettingOffered(snapshot, 'agents.auto_allow.claude');
	return (
		<>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupAfterRestart)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageResumeAgentsDescription)}
					effect='next-restart'
					label={resumeLabel}
					modified={resume.modified}
					query={query}
					settingId='agents.resume-on-restore'
					status={<SettingFailure message={resume.failureMessage} />}
					onRestore={resume.readOnly ? undefined : resume.restore}
				>
					<Switch
						aria-label={resumeLabel}
						checked={resume.value}
						disabled={resume.readOnly}
						onCheckedChange={resume.set}
					/>
				</SettingsRow>
			</SettingsGroup>
			{confirmations ? (
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
			) : null}
		</>
	);
}
