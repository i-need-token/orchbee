import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import { isSettingOnPage } from '@orchbee/workbench-client-protocol';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../../../user-settings';
import { useSetting } from '../../../../hooks/useSetting';
import { SettingFailure } from '../../../setting-failure/SettingFailure';
import { SettingsRow } from '../../../settings-row/SettingsRow';

interface AutomaticUpdatesRowProps {
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Automatic updates, once the settings registry marks that key for the page. */
export function AutomaticUpdatesRow({
	query,
	userSettings,
}: AutomaticUpdatesRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const automaticUpdates = useSetting(userSettings, 'update.automatic');
	if (!isSettingOnPage('update.automatic')) return null;
	const label = intl.formatMessage(terminalSettingsMessages.pageAutomaticUpdatesLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageAutomaticUpdatesDescription)}
			label={label}
			modified={automaticUpdates.modified}
			query={query}
			settingId='about.automatic-updates'
			status={<SettingFailure message={automaticUpdates.failureMessage} />}
			onRestore={automaticUpdates.readOnly ? undefined : automaticUpdates.restore}
		>
			<Switch
				aria-label={label}
				checked={automaticUpdates.value}
				disabled={automaticUpdates.readOnly}
				onCheckedChange={automaticUpdates.set}
			/>
		</SettingsRow>
	);
}
