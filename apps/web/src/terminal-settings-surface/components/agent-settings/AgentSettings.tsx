import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface AgentSettingsProps {
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Whether Agent conversations resume after a restart. */
export function AgentSettings({ query, userSettings }: AgentSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const resume = useSetting(userSettings, 'agents.resume_on_restore');
	const resumeLabel = intl.formatMessage(terminalSettingsMessages.pageResumeAgentsLabel);
	return (
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
	);
}
