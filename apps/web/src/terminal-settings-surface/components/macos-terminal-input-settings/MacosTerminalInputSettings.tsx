import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { isTerminalOptionAsAltMode, TerminalOptionAsAltMode } from '@orchbee/terminal';
import { NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import {
	type MacosTerminalInputSettingsController,
	useMacosTerminalInputSettings,
} from '../../../macos-terminal-input-settings';
import { SettingsRow } from '../settings-row/SettingsRow';

interface MacosTerminalInputSettingsProps {
	readonly controller: MacosTerminalInputSettingsController;
	readonly query: string;
}

/** Which Option keys send Alt combinations in terminals on macOS. */
export function MacosTerminalInputSettings({
	controller,
	query,
}: MacosTerminalInputSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useMacosTerminalInputSettings(controller);
	const label = intl.formatMessage(terminalSettingsMessages.pageOptionAsAltLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageOptionAsAltDescription)}
			label={label}
			modified={snapshot.preferences.optionAsAlt !== TerminalOptionAsAltMode.off}
			query={query}
			settingId='terminal.macos-option-as-alt'
			onRestore={() => controller.update({ optionAsAlt: TerminalOptionAsAltMode.off })}
		>
			<NativeSelect
				aria-label={label}
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
