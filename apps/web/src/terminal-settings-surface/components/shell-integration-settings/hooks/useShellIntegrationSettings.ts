import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';
import type { ChangeEventHandler, FocusEventHandler, KeyboardEventHandler } from 'react';

import {
	MAXIMUM_COMPLETION_REMINDER_SECONDS,
	MINIMUM_COMPLETION_REMINDER_SECONDS,
	TerminalCompletionReminderMode,
	type TerminalCompletionReminderModeValue,
	type TerminalFeedbackSettingsController,
	type TerminalNotificationPermission,
} from '../../../../terminal-feedback-settings';
import { SHELL_INTEGRATION_SETUP_COMMANDS } from '../constants/shell-integration-setup';

interface ShellSetupCardView {
	readonly command: string;
	readonly commandLabel: string;
	readonly description?: string | undefined;
	readonly id: 'bash' | 'fish' | 'zsh';
	readonly title: string;
}

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
	readonly description: string;
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
	readonly manualNotice: string | null;
	readonly noResults: string | null;
	readonly notification: NotificationSettingsView | null;
	readonly persistenceWarning: string | null;
	readonly reminder: ReminderSettingsView | null;
	readonly reset: ResetSettingsView | null;
	readonly setupCards: readonly ShellSetupCardView[];
}

export function useShellIntegrationSettings(
	controller: TerminalFeedbackSettingsController,
	query: string,
): ShellIntegrationSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const [minimumDraft, setMinimumDraft] = React.useState(
		String(snapshot.preferences.completionReminderMinimumSeconds),
	);
	const [minimumInvalid, setMinimumInvalid] = React.useState(false);
	React.useEffect(() => {
		setMinimumDraft(String(snapshot.preferences.completionReminderMinimumSeconds));
		setMinimumInvalid(false);
	}, [snapshot.preferences.completionReminderMinimumSeconds]);

	const messages = {
		bashTitle: intl.formatMessage(terminalSettingsMessages.shellIntegrationBashTitle),
		bashVersion: intl.formatMessage(terminalSettingsMessages.shellIntegrationBashVersion),
		description: intl.formatMessage(terminalSettingsMessages.shellIntegrationDescription),
		fishTitle: intl.formatMessage(terminalSettingsMessages.shellIntegrationFishTitle),
		manualNotice: intl.formatMessage(terminalSettingsMessages.shellIntegrationManualNotice),
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
		noResults: intl.formatMessage(terminalSettingsMessages.noResults),
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
		setupCommandLabel: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationSetupCommandLabel,
		),
		title: intl.formatMessage(terminalSettingsMessages.shellIntegrationTitle),
		zshTitle: intl.formatMessage(terminalSettingsMessages.shellIntegrationZshTitle),
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
	const normalizedQuery = query.trim().toLocaleLowerCase(intl.locale);
	const matches = (...values: readonly string[]): boolean =>
		normalizedQuery.length === 0 ||
		values.some((value) => value.toLocaleLowerCase(intl.locale).includes(normalizedQuery));
	const showAll = matches(messages.title, messages.description, messages.manualNotice);
	const setupCards: ShellSetupCardView[] = [];
	if (
		showAll ||
		matches(messages.zshTitle, messages.setupCommandLabel, SHELL_INTEGRATION_SETUP_COMMANDS.zsh)
	) {
		setupCards.push({
			command: SHELL_INTEGRATION_SETUP_COMMANDS.zsh,
			commandLabel: messages.setupCommandLabel,
			id: 'zsh',
			title: messages.zshTitle,
		});
	}
	if (
		showAll ||
		matches(
			messages.bashTitle,
			messages.bashVersion,
			messages.setupCommandLabel,
			SHELL_INTEGRATION_SETUP_COMMANDS.bash,
		)
	) {
		setupCards.push({
			command: SHELL_INTEGRATION_SETUP_COMMANDS.bash,
			commandLabel: messages.setupCommandLabel,
			description: messages.bashVersion,
			id: 'bash',
			title: messages.bashTitle,
		});
	}
	if (
		showAll ||
		matches(messages.fishTitle, messages.setupCommandLabel, SHELL_INTEGRATION_SETUP_COMMANDS.fish)
	) {
		setupCards.push({
			command: SHELL_INTEGRATION_SETUP_COMMANDS.fish,
			commandLabel: messages.setupCommandLabel,
			id: 'fish',
			title: messages.fishTitle,
		});
	}
	const reminderVisible =
		showAll ||
		matches(
			messages.reminderTitle,
			messages.modeLabel,
			messages.modeDescription,
			messages.modeNever,
			messages.modeUnfocused,
			messages.modeAlways,
			messages.minimumLabel,
			messages.minimumDescription,
		);
	const notificationVisible =
		showAll ||
		matches(
			messages.notificationTitle,
			messages.notificationDescription,
			notificationStatus,
			messages.notificationRequest,
		);
	const resetVisible = showAll || matches(messages.reset, messages.resetDescription);
	const commitMinimum = (): void => {
		const result = controller.update({
			completionReminderMinimumSeconds: Number(minimumDraft),
		});
		setMinimumInvalid(result.kind === 'invalid');
	};
	const onMinimumChange: ChangeEventHandler<HTMLInputElement> = (event) =>
		setMinimumDraft(event.currentTarget.value);
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
	const onReset = (): void => {
		controller.reset();
	};
	const reminder: ReminderSettingsView | null = reminderVisible
		? {
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
			}
		: null;
	const notification: NotificationSettingsView | null = notificationVisible
		? {
				description: messages.notificationDescription,
				onRequestPermission:
					snapshot.notificationPermission === 'prompt' ? onRequestPermission : null,
				permission: snapshot.notificationPermission,
				requestLabel: messages.notificationRequest,
				status: notificationStatus,
				title: messages.notificationTitle,
			}
		: null;
	const reset: ResetSettingsView | null = resetVisible
		? {
				description: messages.resetDescription,
				label: messages.reset,
				onReset,
			}
		: null;

	return {
		manualNotice: showAll ? messages.manualNotice : null,
		noResults:
			setupCards.length === 0 && reminder === null && notification === null && reset === null
				? messages.noResults
				: null,
		notification,
		persistenceWarning,
		reminder,
		reset,
		setupCards,
	};
}
