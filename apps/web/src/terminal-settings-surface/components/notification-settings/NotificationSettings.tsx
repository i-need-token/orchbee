import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { NativeSelect, NativeSelectOption, Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import {
	AgentNotificationMode,
	TerminalCompletionReminderMode,
	type TerminalFeedbackSettingsController,
} from '../../../terminal-feedback-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
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
	const agentMode = useSetting(controller.userSettings, 'notifications.agent_reminder');
	const reminderMode = useSetting(controller.userSettings, 'notifications.completion_reminder');
	const sound = useSetting(controller.userSettings, 'notifications.sound');
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
					status={<SettingFailure message={agentMode.failureMessage} />}
					onRestore={agentMode.readOnly ? undefined : agentMode.restore}
				>
					<NativeSelect
						aria-label={agentLabel}
						className={styles['select']}
						disabled={agentMode.readOnly}
						size='sm'
						value={agentMode.value}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							if (value === AgentNotificationMode.hidden || value === AgentNotificationMode.never) {
								agentMode.set(value);
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
					status={<SettingFailure message={reminderMode.failureMessage} />}
					onRestore={reminderMode.readOnly ? undefined : reminderMode.restore}
				>
					<NativeSelect
						aria-label={reminderLabel}
						className={styles['select']}
						disabled={reminderMode.readOnly}
						size='sm'
						value={reminderMode.value}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							switch (value) {
								case TerminalCompletionReminderMode.always:
								case TerminalCompletionReminderMode.never:
								case TerminalCompletionReminderMode.unfocused:
									reminderMode.set(value);
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
					status={<SettingFailure message={sound.failureMessage} />}
					onRestore={sound.readOnly ? undefined : sound.restore}
				>
					<Switch
						aria-label={soundLabel}
						checked={sound.value}
						disabled={sound.readOnly}
						onCheckedChange={sound.set}
					/>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}
