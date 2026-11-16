import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
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
import { SettingsSelect } from '../settings-select/SettingsSelect';

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
					<SettingsSelect
						disabled={agentMode.readOnly}
						label={agentLabel}
						options={[
							{
								label: intl.formatMessage(terminalSettingsMessages.pageAgentNotificationsNever),
								value: AgentNotificationMode.never,
							},
							{
								label: intl.formatMessage(terminalSettingsMessages.pageAgentNotificationsHidden),
								value: AgentNotificationMode.hidden,
							},
						]}
						value={agentMode.value}
						onValueChange={(value): void => {
							agentMode.set(value);
						}}
					/>
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
					<SettingsSelect
						disabled={reminderMode.readOnly}
						label={reminderLabel}
						options={[
							{
								label: intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersNever),
								value: TerminalCompletionReminderMode.never,
							},
							{
								label: intl.formatMessage(
									terminalSettingsMessages.pageCompletionRemindersUnfocused,
								),
								value: TerminalCompletionReminderMode.unfocused,
							},
							{
								label: intl.formatMessage(terminalSettingsMessages.pageCompletionRemindersAlways),
								value: TerminalCompletionReminderMode.always,
							},
						]}
						value={reminderMode.value}
						onValueChange={(value): void => {
							reminderMode.set(value);
						}}
					/>
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
