import { terminalSettingsMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import { USER_PREFERENCE_KEYS } from '@orchbee/workbench-client-protocol';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import type { UserPreferencesControl } from '../../../user-preferences';
import styles from './SharedPreferenceStatus.module.css';

export function SharedPreferenceStatus({
	control,
}: {
	readonly control: UserPreferencesControl;
}): React.ReactNode {
	const intl = useOrchbeeIntl();
	const state = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const [confirming, setConfirming] = React.useState(false);
	let descriptor: MessageDescriptor = terminalSettingsMessages.sharedScope;
	if (state.pending) descriptor = terminalSettingsMessages.sharedPending;
	else if (state.failure === 'outcome-unknown') descriptor = terminalSettingsMessages.sharedUnknown;
	else if (state.failure === 'source-changed') descriptor = terminalSettingsMessages.sharedConflict;
	else if (!state.available) descriptor = terminalSettingsMessages.sharedUnavailable;
	else if (state.failure !== null) descriptor = terminalSettingsMessages.sharedFailed;
	else if (state.applicationIssue !== null) descriptor = terminalSettingsMessages.sharedHost;
	return (
		<div className={styles['root']} data-slot='shared-preference-status'>
			<p role='status'>{intl.formatMessage(descriptor)}</p>
			<div className={styles['actions']}>
				<Button
					disabled={state.pending}
					size='sm'
					variant='outline'
					onClick={() => {
						void control.refresh();
					}}
				>
					{intl.formatMessage(terminalSettingsMessages.sharedRefresh)}
				</Button>
				{confirming ? (
					<>
						<span>{intl.formatMessage(terminalSettingsMessages.sharedConfirmReset)}</span>
						<Button
							disabled={state.pending || !state.available}
							size='sm'
							variant='destructive'
							onClick={() => {
								setConfirming(false);
								void control.reset(USER_PREFERENCE_KEYS);
							}}
						>
							{intl.formatMessage(terminalSettingsMessages.sharedReset)}
						</Button>
						<Button size='sm' variant='ghost' onClick={() => setConfirming(false)}>
							{intl.formatMessage(terminalSettingsMessages.profilesCancel)}
						</Button>
					</>
				) : (
					<Button
						disabled={state.pending || !state.available}
						size='sm'
						variant='ghost'
						onClick={() => setConfirming(true)}
					>
						{intl.formatMessage(terminalSettingsMessages.sharedReset)}
					</Button>
				)}
			</div>
		</div>
	);
}
