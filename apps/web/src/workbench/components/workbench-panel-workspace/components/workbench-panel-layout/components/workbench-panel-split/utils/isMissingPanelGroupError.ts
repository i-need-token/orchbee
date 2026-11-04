export function isMissingPanelGroupError(error: unknown): boolean {
	return error instanceof Error && error.message.startsWith('Could not find Group with id ');
}
