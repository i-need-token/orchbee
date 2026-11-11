import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { NativeSelect, NativeSelectOption, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import {
	AgentNotificationMode,
	TerminalCompletionReminderMode,
	type TerminalFeedbackSettingsController,
} from '../../../terminal-feedback-settings';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import styles from './NotificationSettings.module.css';

interface NotificationSettingsProps {
	readonly controller: TerminalFeedbackSettingsController;
	readonly query: string;
}

/** When Orchbee notifies about Agents and finished commands, and whether it plays a sound. */
export function NotificationSettings({
	controller,
	query,
}: NotificationSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useFeedbackSnapshot(controller);
	const agentMode = useSharedPreferenceDefault(controller.userPreferences, 'agentNotificationMode');
	const reminderMode = useSharedPreferenceDefault(
		controller.userPreferences,
		'completionReminderMode',
	);
	const sound = useSharedPreferenceDefault(controller.userPreferences, 'soundEnabled');
	const agentLabel = intl.formatMessage(terminalSettingsMessages.pageAgentNotificationsLabel);
	const reminderLabel = intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersLabel);
	const soundLabel = intl.formatMessage(terminalSettingsMessages.pageSoundLabel);
	return (
		<>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupWhenToNotify)}>
				<SettingsRow
					description={intl.formatMessage(
						terminalSettingsMessages.pageAgentNotificationsDescription,
					)}
					label={agentLabel}
					modified={agentMode.modified}
					query={query}
					settingId='notifications.agent-mode'
					onRestore={agentMode.restore}
				>
					<NativeSelect
						aria-label={agentLabel}
						className={styles['select']}
						size='sm'
						value={snapshot.preferences.agentNotificationMode}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							if (value === AgentNotificationMode.hidden || value === AgentNotificationMode.never) {
								controller.update({ agentNotificationMode: value });
							}
						}}
					>
						<NativeSelectOption value={AgentNotificationMode.never}>
							{intl.formatMessage(terminalSettingsMessages.pageAgentNotificationsNever)}
						</NativeSelectOption>
						<NativeSelectOption value={AgentNotificationMode.hidden}>
							{intl.formatMessage(terminalSettingsMessages.pageAgentNotificationsHidden)}
						</NativeSelectOption>
					</NativeSelect>
				</SettingsRow>
				<SettingsRow
					description={intl.formatMessage(
						terminalSettingsMessages.pageCompletionRemindersDescription,
					)}
					label={reminderLabel}
					modified={reminderMode.modified}
					query={query}
					settingId='notifications.completion-reminders'
					onRestore={reminderMode.restore}
				>
					<NativeSelect
						aria-label={reminderLabel}
						className={styles['select']}
						size='sm'
						value={snapshot.preferences.completionReminderMode}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							switch (value) {
								case TerminalCompletionReminderMode.always:
								case TerminalCompletionReminderMode.never:
								case TerminalCompletionReminderMode.unfocused:
									controller.update({ completionReminderMode: value });
							}
						}}
					>
						<NativeSelectOption value={TerminalCompletionReminderMode.never}>
							{intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersNever)}
						</NativeSelectOption>
						<NativeSelectOption value={TerminalCompletionReminderMode.unfocused}>
							{intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersUnfocused)}
						</NativeSelectOption>
						<NativeSelectOption value={TerminalCompletionReminderMode.always}>
							{intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersAlways)}
						</NativeSelectOption>
					</NativeSelect>
				</SettingsRow>
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupSoundAndDelivery)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageSoundDescription)}
					label={soundLabel}
					modified={sound.modified}
					query={query}
					settingId='notifications.sound'
					onRestore={sound.restore}
				>
					<Switch
						aria-label={soundLabel}
						checked={snapshot.preferences.soundEnabled}
						onCheckedChange={(checked): void => {
							controller.update({ soundEnabled: checked });
						}}
					/>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
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
