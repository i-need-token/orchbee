import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { AgentIntegrationManagementControl } from '../../../agent-integration-management';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { AgentIntegrationSettings } from '../agent-integration-settings/AgentIntegrationSettings';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface AgentSettingsProps {
	readonly integrationControl?: AgentIntegrationManagementControl | undefined;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Whether Agent conversations resume after a restart, and each Agent's integration. */
export function AgentSettings({
	integrationControl,
	query,
	userSettings,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const resume = useSetting(userSettings, 'agents.resume_on_restore');
	const resumeLabel = intl.formatMessage(terminalSettingsMessages.pageResumeAgentsLabel);
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
			{integrationControl === undefined ? null : (
				<AgentIntegrationSettings control={integrationControl} query={query} />
			)}
		</>
	);
}
