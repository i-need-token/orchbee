import type { TabGroupId } from '../model/tab-types';

export interface DragPoint {
	x: number;
	y: number;
}

export function calculateHorizontalMoveThreshold(
	targetTabWidth: number,
	standardTabWidth: number,
): number {
	return Math.round((targetTabWidth / standardTabWidth) * 16);
}
export function shouldReorderDraggedTabs(input: {
	justAttached: boolean;
	movementSinceLastReorder: number;
	threshold: number;
	initialMove: boolean;
	tabsConsecutive: boolean;
}): boolean {
	return (
		input.justAttached ||
		Math.abs(input.movementSinceLastReorder) > input.threshold ||
		(input.initialMove && !input.tabsConsecutive)
	);
}
interface DragGroupCalculationInput {
	containsGroupHeader: boolean;
	hasSelectedUnpinnedTab: boolean;
	leftGroup: TabGroupId | null;
	rightGroup: TabGroupId | null;
	currentGroup: TabGroupId | null;
	leftGroupCollapsed: boolean;
	rightGroupCollapsed: boolean;
	selectedTabWidth: number;
	tabLeftInset: number;
	leftEdge: number;
	selectedLeft: number;
	selectedRight: number;
	tabDragAreaEnd: number;
}

/** Pure port of DraggingTabsSession::CalculateGroupForDraggedTabs. */
export function calculateGroupForDraggedTabs(input: DragGroupCalculationInput): TabGroupId | null {
	if (input.containsGroupHeader || !input.hasSelectedUnpinnedTab) return null;
	if (input.leftGroup === input.rightGroup) return input.leftGroup;
	const buffer = Math.trunc(input.selectedTabWidth / 4);
	let leftEdge = input.leftEdge;
	if (input.leftGroup !== null && input.leftGroup === input.currentGroup) leftEdge += buffer;
	if (
		input.rightGroup !== null &&
		input.rightGroup === input.currentGroup &&
		leftEdge > input.tabLeftInset
	) {
		leftEdge -= buffer;
	}
	if (input.leftGroup !== null && !input.leftGroupCollapsed) {
		if (input.selectedRight >= input.tabDragAreaEnd) return null;
		if (input.selectedLeft <= leftEdge - buffer) return input.leftGroup;
	}
	if (
		input.selectedLeft >= leftEdge + buffer &&
		input.rightGroup !== null &&
		!input.rightGroupCollapsed
	) {
		return input.rightGroup;
	}
	return null;
}
