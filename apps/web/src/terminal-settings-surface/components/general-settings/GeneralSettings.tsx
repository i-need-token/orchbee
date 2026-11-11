import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { ApplicationLocaleController } from '../../../application-locale';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { ApplicationLanguageSettings } from '../application-language-settings/ApplicationLanguageSettings';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface GeneralSettingsProps {
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Language, and whether terminal output comes back after a restart. */
export function GeneralSettings({
	applicationLocaleController,
	query,
	userSettings,
}: GeneralSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const restoreOutput = useSetting(userSettings, 'terminal.restore_output');
	const restoreOutputLabel = intl.formatMessage(terminalSettingsMessages.pageRestoreOutputLabel);
	return (
		<>
			<SettingsGroup>
				<ApplicationLanguageSettings
					controller={applicationLocaleController}
					query={query}
					userSettings={userSettings}
				/>
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupAfterRestart)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageRestoreOutputDescription)}
					effect='next-restart'
					label={restoreOutputLabel}
					modified={restoreOutput.modified}
					query={query}
					settingId='general.restore-terminal-output'
					status={<SettingFailure message={restoreOutput.failureMessage} />}
					onRestore={restoreOutput.readOnly ? undefined : restoreOutput.restore}
				>
					<Switch
						aria-label={restoreOutputLabel}
						checked={restoreOutput.value}
						disabled={restoreOutput.readOnly}
						onCheckedChange={restoreOutput.set}
					/>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}
