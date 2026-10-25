import type {
	HorizontalGroupDragAuthoritativeState,
	HorizontalGroupDragState,
} from '../horizontal-group-drag-machine';

type HorizontalGroupDragActiveSurface =
	| {
			readonly activeTabId: string;
			readonly id: string;
			readonly kind: 'group-header';
	  }
	| {
			readonly activeTabId: string;
			readonly id: string;
			readonly kind: 'tab';
	  };

export function getHorizontalGroupDragActiveSurface(
	state: Readonly<Pick<HorizontalGroupDragAuthoritativeState, 'activeTabId' | 'groups' | 'tabs'>>,
): HorizontalGroupDragActiveSurface | null {
	if (state.activeTabId === null) return null;
	const tab = state.tabs.find((candidate) => candidate.id === state.activeTabId);
	if (!tab) return null;
	if (tab.group === null) {
		return {
			kind: 'tab',
			id: tab.id,
			activeTabId: tab.id,
		};
	}
	const group = state.groups.find((candidate) => candidate.id === tab.group);
	if (!group) return null;
	return group.collapsed
		? {
				kind: 'group-header',
				id: group.id,
				activeTabId: tab.id,
			}
		: {
				kind: 'tab',
				id: tab.id,
				activeTabId: tab.id,
			};
}

export function getHorizontalGroupDragInvariantViolations(
	state: Readonly<HorizontalGroupDragState>,
): readonly string[] {
	const violations: string[] = [];
	const ids = new Set<string>();
	const knownGroups = new Set(state.authoritative.groups.map((group) => group.id));
	let sawUnpinned = false;
	const closedGroups = new Set<string>();
	let previousGroup: string | null = null;
	for (const tab of state.authoritative.tabs) {
		if (ids.has(tab.id)) violations.push('UniqueTabIdentity');
		ids.add(tab.id);
		if (!tab.pinned) sawUnpinned = true;
		else if (sawUnpinned || tab.group !== null) violations.push('PinnedPrefix');
		if (tab.group !== null && !knownGroups.has(tab.group)) violations.push('KnownGroupMembership');
		if (previousGroup !== tab.group && previousGroup !== null) closedGroups.add(previousGroup);
		if (tab.group !== null && closedGroups.has(tab.group)) violations.push('GroupContiguous');
		previousGroup = tab.group;
	}
	if (
		(state.authoritative.tabs.length > 0 && state.authoritative.activeTabId === null) ||
		(state.authoritative.activeTabId !== null && !ids.has(state.authoritative.activeTabId))
	) {
		violations.push('ActiveTabExists');
	}
	if (state.authoritative.selectedTabIds.some((id) => !ids.has(id))) {
		violations.push('SelectionExists');
	}
	const stableProjection = state.drag.kind !== 'dragging';
	if (
		stableProjection &&
		state.authoritative.tabs.length > 0 &&
		getHorizontalGroupDragActiveSurface(state.authoritative) === null
	) {
		violations.push('StableActiveSurfaceExists');
	}
	if (state.drag.kind !== 'idle') {
		const drag = state.drag;
		const currentMembers = state.authoritative.tabs
			.filter((tab) => tab.group === drag.groupId)
			.map((tab) => tab.id);
		if (
			!knownGroups.has(drag.groupId) ||
			currentMembers.join('\u0000') !== drag.memberIds.join('\u0000')
		) {
			violations.push('DragSourceExists');
			violations.push('GroupMoveAtomic');
		}
		const group = state.authoritative.groups.find((candidate) => candidate.id === drag.groupId);
		if (drag.wasCollapsedBeforeDrag && group?.collapsed !== true) {
			violations.push('CollapsedDragStaysCompact');
		}
		if (!drag.wasCollapsedBeforeDrag && drag.visualKind !== 'expanded-group') {
			violations.push('ExpandedDragKeepsFullVisual');
		}
		if (
			drag.kind === 'dragging' &&
			!drag.wasCollapsedBeforeDrag &&
			drag.collapseExpandedBackingModel &&
			group?.collapsed !== true
		) {
			violations.push('ExpandedDragKeepsFullVisual');
		}
		if (drag.kind === 'settling') {
			if (group?.collapsed !== drag.wasCollapsedBeforeDrag) {
				violations.push('CompletionRestoresCollapsedIntent');
			}
			if (
				state.authoritative.activeTabId !== drag.recordedActiveTabId ||
				state.authoritative.selectedTabIds.length !== 1 ||
				state.authoritative.selectedTabIds[0] !== drag.recordedActiveTabId
			) {
				violations.push('CompletionSelectsRecordedActive');
			}
		}
	}
	return [...new Set(violations)];
}
