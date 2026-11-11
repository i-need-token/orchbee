import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalFeedbackSettingsController } from '../../../terminal-feedback-settings';
import { SettingsBanner } from '../settings-banner/SettingsBanner';
import styles from './NotificationPermissionBanner.module.css';

interface NotificationPermissionBannerProps {
	readonly controller: TerminalFeedbackSettingsController;
	readonly onDismiss: () => void;
}

/** The page banner while the system has not allowed Orchbee's notifications. */
export function NotificationPermissionBanner({
	controller,
	onDismiss,
}: NotificationPermissionBannerProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { notificationPermission } = useFeedbackSnapshot(controller);
	if (notificationPermission === 'prompt') {
		return (
			<SettingsBanner
				action={
					<Button
						className={styles['allow']}
						size='sm'
						type='button'
						onClick={() => {
							void controller.requestNotificationPermission();
						}}
					>
						{intl.formatMessage(terminalSettingsMessages.pagePermissionAllow)}
					</Button>
				}
				description={intl.formatMessage(terminalSettingsMessages.pagePermissionAskDescription)}
				title={intl.formatMessage(terminalSettingsMessages.pagePermissionAskTitle)}
				onDismiss={onDismiss}
			/>
		);
	}
	if (notificationPermission === 'denied') {
		return (
			<SettingsBanner
				description={intl.formatMessage(terminalSettingsMessages.pagePermissionDeniedDescription)}
				title={intl.formatMessage(terminalSettingsMessages.pagePermissionDeniedTitle)}
				onDismiss={onDismiss}
			/>
		);
	}
	return null;
}

function useFeedbackSnapshot(
	controller: TerminalFeedbackSettingsController,
): ReturnType<TerminalFeedbackSettingsController['getSnapshot']> {
	return React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
}
