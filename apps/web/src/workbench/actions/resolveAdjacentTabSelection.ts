export interface AdjacentTabSelectionInput {
	readonly activeTabId: string | null;
	readonly direction: 'next' | 'previous';
	readonly tabIds: readonly string[];
}

export function resolveAdjacentTabSelection(input: AdjacentTabSelectionInput): string | null {
	if (input.activeTabId === null || input.tabIds.length < 2) return null;
	const activeIndex = input.tabIds.indexOf(input.activeTabId);
	if (activeIndex < 0) return null;
	let targetIndex: number;
	if (input.direction === 'previous') {
		targetIndex = activeIndex === 0 ? input.tabIds.length - 1 : activeIndex - 1;
	} else {
		targetIndex = activeIndex === input.tabIds.length - 1 ? 0 : activeIndex + 1;
	}
	return input.tabIds[targetIndex] ?? null;
}
