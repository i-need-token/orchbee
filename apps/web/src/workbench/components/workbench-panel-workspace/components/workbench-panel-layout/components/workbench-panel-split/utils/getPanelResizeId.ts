export function getPanelResizeId(splitId: string, childId: string): string {
	return `${splitId}--${childId}`;
}
