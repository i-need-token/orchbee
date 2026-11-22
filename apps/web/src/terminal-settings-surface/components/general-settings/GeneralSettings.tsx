import type * as React from 'react';

import type { ApplicationLocaleController } from '../../../application-locale';
import type { UserSettingsControl } from '../../../user-settings';
import { ApplicationLanguageSettings } from '../application-language-settings/ApplicationLanguageSettings';
import { SettingsGroup } from '../settings-group/SettingsGroup';

interface GeneralSettingsProps {
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** The application language. */
export function GeneralSettings({
	applicationLocaleController,
	query,
	userSettings,
}: GeneralSettingsProps): React.ReactNode {
	return (
		<SettingsGroup>
			<ApplicationLanguageSettings
				controller={applicationLocaleController}
				query={query}
				userSettings={userSettings}
			/>
		</SettingsGroup>
	);
}
