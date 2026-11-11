import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { isTerminalOptionAsAltMode } from '@orchbee/terminal';
import { NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import {
	type MacosTerminalInputSettingsController,
	useMacosTerminalInputSettings,
} from '../../../macos-terminal-input-settings';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsRow } from '../settings-row/SettingsRow';

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
			<NativeSelect
				aria-label={label}
				disabled={setting.readOnly}
				size='sm'
				value={snapshot.preferences.optionAsAlt}
				onChange={(event): void => {
					const mode = event.currentTarget.value;
					if (isTerminalOptionAsAltMode(mode)) controller.update({ optionAsAlt: mode });
				}}
			>
				<NativeSelectOption value='off'>
					{intl.formatMessage(terminalSettingsMessages.pageOptionAsAltOff)}
				</NativeSelectOption>
				<NativeSelectOption value='left'>
					{intl.formatMessage(terminalSettingsMessages.pageOptionAsAltLeft)}
				</NativeSelectOption>
				<NativeSelectOption value='right'>
					{intl.formatMessage(terminalSettingsMessages.pageOptionAsAltRight)}
				</NativeSelectOption>
				<NativeSelectOption value='both'>
					{intl.formatMessage(terminalSettingsMessages.pageOptionAsAltBoth)}
				</NativeSelectOption>
			</NativeSelect>
		</SettingsRow>
	);
}
