interface ActiveTabMovementTab {
	readonly id: string;
	readonly pinned: boolean;
}

export interface ActiveTabMovementInput {
	readonly activeTabId: string | null;
	readonly direction: 'backward' | 'forward';
	readonly tabs: readonly ActiveTabMovementTab[];
}

export interface ActiveTabMovementDecision {
	readonly direction: 'first' | 'last' | 'next' | 'previous';
	readonly tabId: string;
}

export function resolveActiveTabMovement(
	input: ActiveTabMovementInput,
): ActiveTabMovementDecision | null {
	if (input.activeTabId === null) return null;
	const activeTab = input.tabs.find((tab) => tab.id === input.activeTabId);
	if (activeTab === undefined) return null;
	const partition = input.tabs.filter((tab) => tab.pinned === activeTab.pinned);
	if (partition.length < 2) return null;
	const activeIndex = partition.findIndex((tab) => tab.id === activeTab.id);
	if (activeIndex < 0) return null;
	if (input.direction === 'backward') {
		return {
			direction: activeIndex === 0 ? 'last' : 'previous',
			tabId: activeTab.id,
		};
	}
	return {
		direction: activeIndex === partition.length - 1 ? 'first' : 'next',
		tabId: activeTab.id,
	};
}
