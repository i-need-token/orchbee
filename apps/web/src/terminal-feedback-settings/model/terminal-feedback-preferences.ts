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
	readonly soundEnabled: boolean;
}
