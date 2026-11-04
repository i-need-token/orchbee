export function getPanelSeparatorId(
	splitId: string,
	leadingChildId: string,
	trailingChildId: string,
): string {
	return `${splitId}--${leadingChildId}--${trailingChildId}`;
}
