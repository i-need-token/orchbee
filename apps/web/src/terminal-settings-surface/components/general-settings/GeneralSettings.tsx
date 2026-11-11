import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { ApplicationLocaleController } from '../../../application-locale';
import type { UserPreferencesControl } from '../../../user-preferences';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { ApplicationLanguageSettings } from '../application-language-settings/ApplicationLanguageSettings';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface GeneralSettingsProps {
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly query: string;
	readonly userPreferences: UserPreferencesControl;
}

/** Language, and whether terminal output comes back after a restart. */
export function GeneralSettings({
	applicationLocaleController,
	query,
	userPreferences,
}: GeneralSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const restoreOutput = useSharedPreferenceDefault(userPreferences, 'restoreTerminalOutput');
	const restoreOutputLabel = intl.formatMessage(terminalSettingsMessages.pageRestoreOutputLabel);
	return (
		<>
			<SettingsGroup>
				<ApplicationLanguageSettings
					controller={applicationLocaleController}
					query={query}
					userPreferences={userPreferences}
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
					onRestore={restoreOutput.restore}
				>
					<Switch
						aria-label={restoreOutputLabel}
						checked={restoreOutput.value}
						onCheckedChange={(checked): void => {
							void userPreferences.update({ restoreTerminalOutput: checked });
						}}
					/>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}
