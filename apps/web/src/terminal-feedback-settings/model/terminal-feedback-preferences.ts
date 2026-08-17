export const TerminalCompletionReminderMode = {
	always: 'always',
	never: 'never',
	unfocused: 'unfocused',
} as const;

export type TerminalCompletionReminderMode =
	(typeof TerminalCompletionReminderMode)[keyof typeof TerminalCompletionReminderMode];

export interface TerminalFeedbackPreferences {
	readonly completionReminderMinimumSeconds: number;
	readonly completionReminderMode: TerminalCompletionReminderMode;
}

export const DEFAULT_TERMINAL_FEEDBACK_PREFERENCES: TerminalFeedbackPreferences = Object.freeze({
	completionReminderMinimumSeconds: 5,
	completionReminderMode: TerminalCompletionReminderMode.never,
});

export const MINIMUM_COMPLETION_REMINDER_SECONDS = 1;
export const MAXIMUM_COMPLETION_REMINDER_SECONDS = 3600;
