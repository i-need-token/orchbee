import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type * as React from 'react';

import {
	type MacosTerminalInputSettingsController,
	useMacosTerminalInputSettings,
} from '../../../macos-terminal-input-settings';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsRow } from '../settings-row/SettingsRow';
import { SettingsSelect } from '../settings-select/SettingsSelect';

interface MacosTerminalInputSettingsProps {
	readonly controller: MacosTerminalInputSettingsController;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Which Option keys send Alt combinations in terminals on macOS. */
export function MacosTerminalInputSettings({
	controller,
	query,
	userSettings,
}: MacosTerminalInputSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useMacosTerminalInputSettings(controller);
	const setting = useSetting(userSettings, 'terminal.macos_option_as_alt');
	const label = intl.formatMessage(terminalSettingsMessages.pageOptionAsAltLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageOptionAsAltDescription)}
			label={label}
			modified={setting.modified}
			query={query}
			settingId='terminal.macos-option-as-alt'
			status={<SettingFailure message={setting.failureMessage} />}
			onRestore={setting.readOnly ? undefined : () => controller.reset()}
		>
			<SettingsSelect
				disabled={setting.readOnly}
				label={label}
				options={[
					{ label: intl.formatMessage(terminalSettingsMessages.pageOptionAsAltOff), value: 'off' },
					{
						label: intl.formatMessage(terminalSettingsMessages.pageOptionAsAltLeft),
						value: 'left',
					},
					{
						label: intl.formatMessage(terminalSettingsMessages.pageOptionAsAltRight),
						value: 'right',
					},
					{
						label: intl.formatMessage(terminalSettingsMessages.pageOptionAsAltBoth),
						value: 'both',
					},
				]}
				value={snapshot.preferences.optionAsAlt}
				onValueChange={(value): void => {
					controller.update({ optionAsAlt: value });
				}}
			/>
		</SettingsRow>
	);
}
