import { agentObservationMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

import styles from './AgentSettings.module.css';

interface AgentSettingsProps {
	readonly query: string;
	/** Opens the page where more Agents are requested; absent where no external browser is wired. */
	readonly requestAgent?: (() => void) | undefined;
	readonly userSettings: UserSettingsControl;
}

/**
 * Whether Agent conversations resume after a restart, and for which Agent types Orchbee answers
 * routine confirmations: one switch for each type it can answer for.
 */
export function AgentSettings({
	query,
	requestAgent,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const resume = useSetting(userSettings, 'agents.resume_on_restore');
	const resumeLabel = intl.formatMessage(terminalSettingsMessages.pageResumeAgentsLabel);
	const allowClaude = useSetting(userSettings, 'agents.auto_allow.claude');
	const claudeLabel = intl.formatMessage(agentObservationMessages.providerClaudeCode);
	const request = intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsRequest);
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
				<SettingsRow
					description={intl.formatMessage(
						terminalSettingsMessages.pageAgentConfirmationsRowDescription,
					)}
					label={claudeLabel}
					modified={allowClaude.modified}
					query={query}
					searchTerms={[
						intl.formatMessage(terminalSettingsMessages.pageGroupAgentConfirmations),
						intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsDescription),
					]}
					settingId='agents.auto-allow.claude'
					status={<SettingFailure message={allowClaude.failureMessage} />}
					onRestore={allowClaude.readOnly ? undefined : allowClaude.restore}
				>
					<Switch
						aria-label={claudeLabel}
						checked={allowClaude.value}
						disabled={allowClaude.readOnly}
						onCheckedChange={allowClaude.set}
					/>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}
