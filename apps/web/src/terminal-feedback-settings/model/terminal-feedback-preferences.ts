/** The values of `notifications.completion_reminder`, in the file's spelling. */
export const TerminalCompletionReminderMode = {
	always: 'always',
	never: 'never',
	unfocused: 'when-unfocused',
} as const;

export type TerminalCompletionReminderMode =
	(typeof TerminalCompletionReminderMode)[keyof typeof TerminalCompletionReminderMode];

/** The values of `notifications.agent_reminder`, in the file's spelling. */
export const AgentNotificationMode = {
	hidden: 'when-hidden',
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
