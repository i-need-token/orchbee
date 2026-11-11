import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { AgentIntegrationManagementControl } from '../../../agent-integration-management';
import type { UserPreferencesControl } from '../../../user-preferences';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { AgentIntegrationSettings } from '../agent-integration-settings/AgentIntegrationSettings';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface AgentSettingsProps {
	readonly integrationControl?: AgentIntegrationManagementControl | undefined;
	readonly query: string;
	readonly userPreferences: UserPreferencesControl;
}

/** Whether Agent conversations resume after a restart, and each Agent's integration. */
export function AgentSettings({
	integrationControl,
	query,
	userPreferences,
}: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const resume = useSharedPreferenceDefault(userPreferences, 'resumeAgentsOnRestore');
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
					onRestore={resume.restore}
				>
					<Switch
						aria-label={resumeLabel}
						checked={resume.value}
						onCheckedChange={(checked): void => {
							void userPreferences.update({ resumeAgentsOnRestore: checked });
						}}
					/>
				</SettingsRow>
			</SettingsGroup>
			{integrationControl === undefined ? null : (
				<AgentIntegrationSettings control={integrationControl} query={query} />
			)}
		</>
	);
}
