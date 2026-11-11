import { terminalSettingsMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import { USER_PREFERENCE_KEYS } from '@orchbee/workbench-client-protocol';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import type { UserPreferencesControl } from '../../../user-preferences';
import { SettingsBanner } from '../settings-banner/SettingsBanner';

interface SharedPreferenceStatusProps {
	readonly control: UserPreferencesControl;
	readonly onDismiss?: (() => void) | undefined;
}

/**
 * A page banner while shared preferences cannot be read or saved. It offers a reload and, for an
 * unreadable record, a confirmed reset of every shared preference; a healthy state shows nothing.
 */
export function SharedPreferenceStatus({
	control,
	onDismiss,
}: SharedPreferenceStatusProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const state = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const [confirming, setConfirming] = React.useState(false);
	let descriptor: MessageDescriptor | null = null;
	if (state.failure === 'outcome-unknown') descriptor = terminalSettingsMessages.sharedUnknown;
	else if (state.failure === 'invalid-source')
		descriptor = terminalSettingsMessages.sharedUnreadable;
	else if (state.failure === 'source-changed') descriptor = terminalSettingsMessages.sharedConflict;
	else if (!state.available) descriptor = terminalSettingsMessages.sharedUnavailable;
	else if (state.failure !== null) descriptor = terminalSettingsMessages.sharedFailed;
	else if (state.applicationIssue !== null) descriptor = terminalSettingsMessages.sharedHost;
	if (descriptor === null) return null;
	const recoverable = state.failure === 'invalid-source';
	return (
		<SettingsBanner
			action={
				<>
					<Button
						disabled={state.pending}
						size='sm'
						type='button'
						variant='outline'
						onClick={() => {
							void control.refresh();
						}}
					>
						{intl.formatMessage(terminalSettingsMessages.sharedRefresh)}
					</Button>
					{recoverable && confirming ? (
						<Button size='sm' type='button' variant='ghost' onClick={() => setConfirming(false)}>
							{intl.formatMessage(terminalSettingsMessages.profilesCancel)}
						</Button>
					) : null}
					{recoverable ? (
						<Button
							disabled={state.pending}
							size='sm'
							type='button'
							variant={confirming ? 'destructive' : 'ghost'}
							onClick={() => {
								if (!confirming) {
									setConfirming(true);
									return;
								}
								setConfirming(false);
								void control.reset(USER_PREFERENCE_KEYS);
							}}
						>
							{intl.formatMessage(terminalSettingsMessages.sharedReset)}
						</Button>
					) : null}
				</>
			}
			description={
				recoverable && confirming
					? intl.formatMessage(terminalSettingsMessages.sharedConfirmReset)
					: undefined
			}
			title={intl.formatMessage(descriptor)}
			onDismiss={onDismiss}
		/>
	);
}
