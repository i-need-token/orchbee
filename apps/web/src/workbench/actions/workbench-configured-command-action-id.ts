/** An Action identity per configured command; the command's own identity changes with its definition. */
const ACTION_ID_PREFIX = 'workbench.configured-command.run:';

export function workbenchConfiguredCommandActionId(commandId: string): string {
	return `${ACTION_ID_PREFIX}${commandId}`;
}

/** The configured command an Action runs, or null for any other Action. */
export function workbenchConfiguredCommandIdOf(actionId: string): string | null {
	return actionId.startsWith(ACTION_ID_PREFIX) ? actionId.slice(ACTION_ID_PREFIX.length) : null;
}
