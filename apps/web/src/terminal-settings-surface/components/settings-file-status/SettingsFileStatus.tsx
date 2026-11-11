import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { useUserSettings } from '../../hooks/useUserSettings';
import { SettingsBanner } from '../settings-banner/SettingsBanner';

interface SettingsFileStatusProps {
	readonly control: UserSettingsControl;
	/** Whether the person dismissed the dismissible notice for this open page. */
	readonly dismissed?: boolean | undefined;
	readonly onDismiss?: (() => void) | undefined;
}

/**
 * One page notice about the settings file. A syntax error or an unreadable file names the file and
 * stays while the page shows the last valid values read-only; an unreachable settings service can
 * be dismissed. A healthy file shows nothing.
 */
export function SettingsFileStatus({
	control,
	dismissed = false,
	onDismiss,
}: SettingsFileStatusProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { available, snapshot } = useUserSettings(control);
	const path = snapshot?.filePath ?? 'config.toml';
	switch (snapshot?.file.status) {
		case 'syntax-error':
			return (
				<SettingsBanner
					title={intl.formatMessage(terminalSettingsMessages.fileSyntaxError, {
						column: snapshot.file.column,
						line: snapshot.file.line,
						path,
					})}
				/>
			);
		case 'unreadable':
			return (
				<SettingsBanner
					title={intl.formatMessage(terminalSettingsMessages.fileUnreadable, { path })}
				/>
			);
		default:
			return available || dismissed ? null : (
				<SettingsBanner
					title={intl.formatMessage(terminalSettingsMessages.sharedUnavailable)}
					onDismiss={onDismiss}
				/>
			);
	}
}
