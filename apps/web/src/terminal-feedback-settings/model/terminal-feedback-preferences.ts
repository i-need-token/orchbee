export const TerminalCompletionReminderMode = {
	always: 'always',
	never: 'never',
	unfocused: 'unfocused',
} as const;

export type TerminalCompletionReminderMode =
	(typeof TerminalCompletionReminderMode)[keyof typeof TerminalCompletionReminderMode];

export const AgentNotificationMode = {
	hidden: 'hidden',
	never: 'never',
} as const;

export type AgentNotificationMode =
	(typeof AgentNotificationMode)[keyof typeof AgentNotificationMode];

export interface TerminalFeedbackPreferences {
	readonly agentNotificationMode: AgentNotificationMode;
	readonly completionReminderMinimumSeconds: number;
	readonly completionReminderMode: TerminalCompletionReminderMode;
}

export const DEFAULT_TERMINAL_FEEDBACK_PREFERENCES: TerminalFeedbackPreferences = Object.freeze({
	agentNotificationMode: AgentNotificationMode.never,
	completionReminderMinimumSeconds: 5,
	completionReminderMode: TerminalCompletionReminderMode.never,
});

export const MINIMUM_COMPLETION_REMINDER_SECONDS = 1;
export const MAXIMUM_COMPLETION_REMINDER_SECONDS = 3600;
