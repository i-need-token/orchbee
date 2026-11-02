import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';
import type { ChangeEventHandler, FocusEventHandler, KeyboardEventHandler } from 'react';

import {
	AgentNotificationMode,
	MAXIMUM_COMPLETION_REMINDER_SECONDS,
	MINIMUM_COMPLETION_REMINDER_SECONDS,
	TerminalCompletionReminderMode,
	type AgentNotificationModeValue,
	type TerminalCompletionReminderModeValue,
	type TerminalFeedbackSettingsController,
	type TerminalNotificationPermission,
} from '../../../../terminal-feedback-settings';
import { useSharedPreferenceDraft } from '../../../hooks/useSharedPreferenceDraft';

interface ReminderModeOptionView {
	readonly label: string;
	readonly value: TerminalCompletionReminderModeValue;
}

interface ReminderSettingsView {
	readonly description: string;
	readonly minimumDescription: string;
	readonly minimumDraft: string;
	readonly minimumInvalid: boolean;
	readonly minimumInvalidMessage: string;
	readonly minimumLabel: string;
	readonly minimumMaximum: number;
	readonly minimumMinimum: number;
	readonly modeLabel: string;
	readonly modeOptions: readonly ReminderModeOptionView[];
	readonly modeValue: TerminalCompletionReminderModeValue;
	readonly onMinimumBlur: FocusEventHandler<HTMLInputElement>;
	readonly onMinimumChange: ChangeEventHandler<HTMLInputElement>;
	readonly onMinimumKeyDown: KeyboardEventHandler<HTMLInputElement>;
	readonly onModeChange: ChangeEventHandler<HTMLSelectElement>;
	readonly title: string;
}

interface NotificationSettingsView {
	readonly soundEnabled: boolean;
	readonly soundLabel: string;
	readonly soundDescription: string;
	readonly onSoundChange: (checked: boolean) => void;
	readonly agentModeLabel: string;
	readonly agentModeOptions: readonly {
		readonly label: string;
		readonly value: AgentNotificationModeValue;
	}[];
	readonly agentModeValue: AgentNotificationModeValue;
	readonly description: string;
	readonly onAgentModeChange: ChangeEventHandler<HTMLSelectElement>;
	readonly onRequestPermission: (() => void) | null;
	readonly permission: TerminalNotificationPermission;
	readonly requestLabel: string;
	readonly status: string;
	readonly title: string;
}

interface ResetSettingsView {
	readonly description: string;
	readonly label: string;
	readonly onReset: () => void;
}

interface ShellIntegrationSettingsView {
	readonly notification: NotificationSettingsView;
	readonly persistenceWarning: string | null;
	readonly reminder: ReminderSettingsView;
	readonly reset: ResetSettingsView;
}

export function useShellIntegrationSettings(
	controller: TerminalFeedbackSettingsController,
): ShellIntegrationSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const minimum = useSharedPreferenceDraft(
		String(snapshot.preferences.completionReminderMinimumSeconds),
		'completionReminderMinimumSeconds',
		controller.userPreferences,
	);
	const minimumDraft = minimum.draft;
	const [minimumInvalid, setMinimumInvalid] = React.useState(false);
	React.useEffect(() => {
		setMinimumInvalid(false);
	}, [snapshot.preferences.completionReminderMinimumSeconds]);

	const messages = {
		minimumDescription: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationReminderMinimumDescription,
		),
		minimumInvalid: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationReminderMinimumInvalid,
			{
				maximum: MAXIMUM_COMPLETION_REMINDER_SECONDS,
				minimum: MINIMUM_COMPLETION_REMINDER_SECONDS,
			},
		),
		minimumLabel: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderMinimumLabel),
		modeAlways: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderAlways),
		modeDescription: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationReminderDescription,
		),
		modeLabel: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderModeLabel),
		modeNever: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderNever),
		modeUnfocused: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderUnfocused),
		notificationAgentModeHidden: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationAgentModeHidden,
		),
		notificationAgentModeLabel: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationAgentModeLabel,
		),
		notificationAgentModeNever: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationAgentModeNever,
		),
		notificationDescription: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationDescription,
		),
		notificationRequest: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationRequest,
		),
		notificationTitle: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationNotificationTitle,
		),
		reminderTitle: intl.formatMessage(terminalSettingsMessages.shellIntegrationReminderTitle),
		reset: intl.formatMessage(terminalSettingsMessages.reset),
		resetDescription: intl.formatMessage(terminalSettingsMessages.shellIntegrationResetDescription),
	};
	const notificationStatus = (() => {
		switch (snapshot.notificationPermission) {
			case 'denied':
				return intl.formatMessage(
					terminalSettingsMessages.shellIntegrationNotificationStatusDenied,
				);
			case 'granted':
				return intl.formatMessage(
					terminalSettingsMessages.shellIntegrationNotificationStatusGranted,
				);
			case 'prompt':
				return intl.formatMessage(
					terminalSettingsMessages.shellIntegrationNotificationStatusPrompt,
				);
			case 'unsupported':
				return intl.formatMessage(
					terminalSettingsMessages.shellIntegrationNotificationStatusUnsupported,
				);
		}
	})();
	const persistenceWarning = (() => {
		switch (snapshot.persistenceIssue) {
			case 'invalid-record':
				return intl.formatMessage(terminalSettingsMessages.persistenceInvalidWarning);
			case 'read-failed':
				return intl.formatMessage(terminalSettingsMessages.persistenceReadWarning);
			case 'write-failed':
				return intl.formatMessage(terminalSettingsMessages.persistenceWriteWarning);
			case null:
				return null;
		}
	})();
	const commitMinimum = (): void => {
		const seconds = Number(minimumDraft);
		if (
			!Number.isInteger(seconds) ||
			seconds < MINIMUM_COMPLETION_REMINDER_SECONDS ||
			seconds > MAXIMUM_COMPLETION_REMINDER_SECONDS
		) {
			setMinimumInvalid(true);
			return;
		}
		setMinimumInvalid(false);
		minimum.submit({ completionReminderMinimumSeconds: seconds }, () => {
			controller.update({ completionReminderMinimumSeconds: seconds });
		});
	};
	const onMinimumChange: ChangeEventHandler<HTMLInputElement> = (event) =>
		minimum.change(event.currentTarget.value);
	const onMinimumKeyDown: KeyboardEventHandler<HTMLInputElement> = (event) => {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		commitMinimum();
		event.currentTarget.blur();
	};
	const onModeChange: ChangeEventHandler<HTMLSelectElement> = (event) => {
		const value = event.currentTarget.value;
		switch (value) {
			case TerminalCompletionReminderMode.always:
			case TerminalCompletionReminderMode.never:
			case TerminalCompletionReminderMode.unfocused:
				controller.update({ completionReminderMode: value });
		}
	};
	const onRequestPermission = (): void => {
		void controller.requestNotificationPermission();
	};
	const onAgentModeChange: ChangeEventHandler<HTMLSelectElement> = (event) => {
		const value = event.currentTarget.value;
		if (value === AgentNotificationMode.hidden || value === AgentNotificationMode.never) {
			controller.update({ agentNotificationMode: value });
		}
	};
	const onReset = (): void => {
		controller.reset();
	};
	const reminder: ReminderSettingsView = {
		description: messages.modeDescription,
		minimumDescription: messages.minimumDescription,
		minimumDraft,
		minimumInvalid,
		minimumInvalidMessage: messages.minimumInvalid,
		minimumLabel: messages.minimumLabel,
		minimumMaximum: MAXIMUM_COMPLETION_REMINDER_SECONDS,
		minimumMinimum: MINIMUM_COMPLETION_REMINDER_SECONDS,
		modeLabel: messages.modeLabel,
		modeOptions: [
			{ label: messages.modeNever, value: TerminalCompletionReminderMode.never },
			{ label: messages.modeUnfocused, value: TerminalCompletionReminderMode.unfocused },
			{ label: messages.modeAlways, value: TerminalCompletionReminderMode.always },
		],
		modeValue: snapshot.preferences.completionReminderMode,
		onMinimumBlur: commitMinimum,
		onMinimumChange,
		onMinimumKeyDown,
		onModeChange,
		title: messages.reminderTitle,
	};
	const notification: NotificationSettingsView = {
		soundEnabled: snapshot.preferences.soundEnabled,
		soundLabel: intl.formatMessage(terminalSettingsMessages.sharedSound),
		soundDescription: intl.formatMessage(terminalSettingsMessages.sharedSoundDescription),
		onSoundChange: (checked) => {
			controller.update({ soundEnabled: checked });
		},
		agentModeLabel: messages.notificationAgentModeLabel,
		agentModeOptions: [
			{ label: messages.notificationAgentModeNever, value: AgentNotificationMode.never },
			{ label: messages.notificationAgentModeHidden, value: AgentNotificationMode.hidden },
		],
		agentModeValue: snapshot.preferences.agentNotificationMode,
		description: messages.notificationDescription,
		onAgentModeChange,
		onRequestPermission: snapshot.notificationPermission === 'prompt' ? onRequestPermission : null,
		permission: snapshot.notificationPermission,
		requestLabel: messages.notificationRequest,
		status: notificationStatus,
		title: messages.notificationTitle,
	};
	const reset: ResetSettingsView = {
		description: messages.resetDescription,
		label: messages.reset,
		onReset,
	};

	return {
		notification,
		persistenceWarning,
		reminder,
		reset,
	};
}
