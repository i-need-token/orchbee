import { reduceWorkbenchTabs } from '../../../../workbench-tabs';
import type { WorkbenchTab, WorkbenchTabsState } from '../../../../workbench-tabs';
import type { WorkbenchTabStripItemGeometry } from '../types/workbench-tab-drag';

export interface WorkbenchTabDragProjection {
	readonly groupId: string | undefined;
	readonly targetIndex: number;
}

interface WorkbenchTabStripIdealItem {
	readonly id: string;
	readonly kind: 'group' | 'tab';
	readonly left: number;
	readonly width: number;
}

interface ProjectionCandidate extends WorkbenchTabDragProjection {
	readonly distance: number;
	readonly state: WorkbenchTabsState;
}

function createWorkbenchTabStripIdealLayout(
	state: WorkbenchTabsState,
	geometry: readonly WorkbenchTabStripItemGeometry[],
): readonly WorkbenchTabStripIdealItem[] {
	const widths = new Map(geometry.map((item) => [`${item.kind}:${item.id}`, item.width]));
	const groupsById = new Map(state.tabGroups.map((group) => [group.id, group]));
	const layout: WorkbenchTabStripIdealItem[] = [];
	const visitedGroupIds = new Set<string>();
	let left = 0;
	const append = (id: string, kind: 'group' | 'tab'): void => {
		const width = widths.get(`${kind}:${id}`);
		if (width === undefined) return;
		layout.push({ id, kind, left, width });
		left += width;
	};

	for (const tab of state.tabs) {
		if (tab.groupId === undefined) {
			append(tab.id, 'tab');
			continue;
		}
		if (visitedGroupIds.has(tab.groupId)) continue;
		visitedGroupIds.add(tab.groupId);
		append(tab.groupId, 'group');
		if (groupsById.get(tab.groupId)?.isCollapsed === true) continue;
		for (const member of state.tabs) {
			if (member.groupId === tab.groupId) append(member.id, 'tab');
		}
	}
	return layout;
}

function getAdjacentGroupCandidates(
	state: WorkbenchTabsState,
	remainingTabs: readonly WorkbenchTab[],
	targetIndex: number,
): readonly (string | undefined)[] {
	const leftGroupId = remainingTabs[targetIndex - 1]?.groupId;
	const rightGroupId = remainingTabs[targetIndex]?.groupId;
	const groupsById = new Map(state.tabGroups.map((group) => [group.id, group]));
	if (leftGroupId !== undefined && leftGroupId === rightGroupId) {
		return groupsById.get(leftGroupId)?.isCollapsed === true ? [] : [leftGroupId];
	}

	const candidates: (string | undefined)[] = [undefined];
	if (leftGroupId !== undefined && groupsById.get(leftGroupId)?.isCollapsed !== true) {
		candidates.unshift(leftGroupId);
	}
	if (
		rightGroupId !== undefined &&
		rightGroupId !== leftGroupId &&
		groupsById.get(rightGroupId)?.isCollapsed !== true
	) {
		candidates.push(rightGroupId);
	}
	return candidates;
}

function getTabCandidateDistance(
	state: WorkbenchTabsState,
	geometry: readonly WorkbenchTabStripItemGeometry[],
	draggedTab: WorkbenchTab,
	draggedLeft: number,
	draggedWidth: number,
	targetIndex: number,
	groupId: string | undefined,
	idealLeftOffset: number,
): ProjectionCandidate | undefined {
	const previewState = reduceWorkbenchTabs(state, {
		groupId,
		kind: 'move-tab',
		tabId: draggedTab.id,
		targetIndex,
	});
	const idealItem = createWorkbenchTabStripIdealLayout(previewState, geometry).find(
		(item) => item.kind === 'tab' && item.id === draggedTab.id,
	);
	if (idealItem === undefined) return undefined;
	const rawDistance = Math.abs(draggedLeft - (idealItem.left + idealLeftOffset));
	const stickyDistance =
		groupId !== undefined && groupId === draggedTab.groupId
			? Math.max(0, rawDistance - draggedWidth / 4)
			: rawDistance;
	return { distance: stickyDistance, groupId, state: previewState, targetIndex };
}

export function resolveWorkbenchTabDragProjection(
	state: WorkbenchTabsState,
	geometry: readonly WorkbenchTabStripItemGeometry[],
	draggedTabId: string,
	draggedCenter: number,
	draggedWidth: number,
): WorkbenchTabDragProjection | undefined {
	const draggedTab = state.tabs.find((tab) => tab.id === draggedTabId);
	if (draggedTab === undefined) return undefined;
	const remainingTabs = state.tabs.filter((tab) => tab.id !== draggedTabId);
	const pinnedCount = remainingTabs.filter((tab) => tab.isPinned === true).length;
	const firstTargetIndex = draggedTab.isPinned === true ? 0 : pinnedCount;
	const lastTargetIndex = draggedTab.isPinned === true ? pinnedCount : remainingTabs.length;
	const draggedLeft = draggedCenter - draggedWidth / 2;
	let bestCandidate: ProjectionCandidate | undefined;

	for (let targetIndex = firstTargetIndex; targetIndex <= lastTargetIndex; targetIndex += 1) {
		const groupCandidates =
			draggedTab.isPinned === true
				? [undefined]
				: getAdjacentGroupCandidates(state, remainingTabs, targetIndex);
		const trailingGroupId = remainingTabs[targetIndex - 1]?.groupId;
		const leadingGroupId = remainingTabs[targetIndex]?.groupId;
		const hasTrailingGroupBoundary =
			trailingGroupId !== undefined &&
			trailingGroupId !== leadingGroupId &&
			groupCandidates.includes(trailingGroupId);
		for (const groupId of groupCandidates) {
			const candidate = getTabCandidateDistance(
				state,
				geometry,
				draggedTab,
				draggedLeft,
				draggedWidth,
				targetIndex,
				groupId,
				groupId === undefined && hasTrailingGroupBoundary ? draggedWidth / 2 : 0,
			);
			if (candidate === undefined) continue;
			if (bestCandidate === undefined || candidate.distance < bestCandidate.distance) {
				bestCandidate = candidate;
			}
		}
	}

	if (bestCandidate === undefined || bestCandidate.state === state) return undefined;
	return { groupId: bestCandidate.groupId, targetIndex: bestCandidate.targetIndex };
}

export function resolveWorkbenchTabGroupDragProjection(
	state: WorkbenchTabsState,
	geometry: readonly WorkbenchTabStripItemGeometry[],
	draggedGroupId: string,
	draggedLeft: number,
): number | undefined {
	const groupTabs = state.tabs.filter((tab) => tab.groupId === draggedGroupId);
	if (groupTabs.length === 0) return undefined;
	const remainingTabs = state.tabs.filter((tab) => tab.groupId !== draggedGroupId);
	const pinnedCount = remainingTabs.filter((tab) => tab.isPinned === true).length;
	let bestCandidate:
		| {
				readonly distance: number;
				readonly state: WorkbenchTabsState;
				readonly targetIndex: number;
		  }
		| undefined;

	for (let targetIndex = pinnedCount; targetIndex <= remainingTabs.length; targetIndex += 1) {
		const leftGroupId = remainingTabs[targetIndex - 1]?.groupId;
		const rightGroupId = remainingTabs[targetIndex]?.groupId;
		if (leftGroupId !== undefined && leftGroupId === rightGroupId) continue;
		const previewState = reduceWorkbenchTabs(state, {
			groupId: draggedGroupId,
			kind: 'move-tab-group',
			targetIndex,
		});
		const idealHeader = createWorkbenchTabStripIdealLayout(previewState, geometry).find(
			(item) => item.kind === 'group' && item.id === draggedGroupId,
		);
		if (idealHeader === undefined) continue;
		const candidate = {
			distance: Math.abs(draggedLeft - idealHeader.left),
			state: previewState,
			targetIndex,
		};
		if (bestCandidate === undefined || candidate.distance < bestCandidate.distance) {
			bestCandidate = candidate;
		}
	}

	if (bestCandidate === undefined || bestCandidate.state === state) return undefined;
	return bestCandidate.targetIndex;
}
