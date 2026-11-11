import { useOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import type { TabStripNewTabChoice, TabStripNewTabSection } from '@orchbee/tab-strip';
import type { UserLaunchProfile } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type { WorkbenchLaunchProfiles } from '../types/workbench-authority';
import { subscribeToNothing } from '../utils/subscribeToNothing';

/**
 * The New Tab button's menu, as in the TUI: every saved launch profile, the default first and marked
 * Default. A choice starts one Tab with its profile and never changes the default. Without a
 * Workspace there is nowhere to put that Tab, so the button offers no menu.
 */
export function useWorkbenchNewTabChoices(
	launchProfiles: WorkbenchLaunchProfiles | undefined,
	inWorkspace: boolean,
	createTab: (profile: UserLaunchProfile) => void,
): readonly TabStripNewTabSection[] | undefined {
	const intl = useOrchbeeIntl();
	const readCatalog = () => launchProfiles?.getSnapshot();
	const catalog = React.useSyncExternalStore(
		launchProfiles?.subscribe ?? subscribeToNothing,
		readCatalog,
		readCatalog,
	);
	return React.useMemo(() => {
		if (catalog === undefined || !inWorkspace) return undefined;
		const { defaultProfileId, profiles } = catalog;
		const choices = [
			...profiles.filter((profile) => profile.id === defaultProfileId),
			...profiles.filter((profile) => profile.id !== defaultProfileId),
		].map(
			(profile): TabStripNewTabChoice => ({
				icon: 'terminal',
				id: profile.id,
				// Settings lists an unnamed profile by what it runs, and so does this menu.
				label:
					profile.name ??
					intl.formatMessage(
						profile.command.kind === 'system-shell'
							? workbenchTabMessages.newTabProfileSystemShell
							: workbenchTabMessages.newTabProfileCustomExecutable,
					),
				...(profile.id === defaultProfileId
					? { note: intl.formatMessage(workbenchTabMessages.newTabProfileDefault) }
					: {}),
				select: () => createTab(profile),
			}),
		);
		return [{ choices, id: 'profiles' }];
	}, [catalog, createTab, inWorkspace, intl]);
}
