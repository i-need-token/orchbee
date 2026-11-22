import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import type { TerminalSettingsAbout } from '../../types/terminal-settings-surface';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { AboutUpdateStatus } from './components/about-update-status/AboutUpdateStatus';
import { AutomaticUpdatesRow } from './components/automatic-updates-row/AutomaticUpdatesRow';

interface AboutSettingsProps {
	/** Absent where this window has no version or update control to show. */
	readonly about: TerminalSettingsAbout | undefined;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/**
 * About: the version, Updates, and the Automatic updates setting in one card. A search lists only
 * the setting, since the version and Updates are information rather than settings.
 */
export function AboutSettings({ about, query, userSettings }: AboutSettingsProps): React.ReactNode {
	const automaticUpdates = <AutomaticUpdatesRow query={query} userSettings={userSettings} />;
	if (about === undefined || query !== '') return <SettingsGroup>{automaticUpdates}</SettingsGroup>;
	return <AboutUpdateStatus about={about}>{automaticUpdates}</AboutUpdateStatus>;
}
