import { requireDefined } from '../utils/requireDefined';
import { moveContiguousBlock } from '../model/moveContiguousBlock';
interface HorizontalGroupDragTab {
	readonly group: string | null;
	readonly id: string;
	readonly pinned: boolean;
}
interface HorizontalGroupDragGroup {
	readonly collapsed: boolean;
	readonly id: string;
}
export interface HorizontalGroupDragAuthoritativeState {
	readonly activeTabId: string | null;
	readonly groups: readonly HorizontalGroupDragGroup[];
	readonly selectedTabIds: readonly string[];
	readonly selectionAnchorId: string | null;
	readonly tabs: readonly HorizontalGroupDragTab[];
}
interface GroupDragSession {
	readonly collapseExpandedBackingModel: boolean;
	readonly groupId: string;
	readonly memberIds: readonly string[];
	readonly original: HorizontalGroupDragAuthoritativeState;
	readonly recordedActiveTabId: string;
	readonly targetIndex: number | null;
	readonly visualKind: 'collapsed-header' | 'expanded-group';
	readonly wasCollapsedBeforeDrag: boolean;
}
type HorizontalGroupDragPhase =
	| {
			readonly kind: 'idle';
	  }
	| (GroupDragSession & {
			readonly kind: 'armed';
	  })
	| (GroupDragSession & {
			readonly kind: 'dragging';
	  })
	| (GroupDragSession & {
			readonly kind: 'settling';
	  });
export interface HorizontalGroupDragState {
	readonly authoritative: HorizontalGroupDragAuthoritativeState;
	readonly drag: HorizontalGroupDragPhase;
}

type HorizontalGroupDragEvent =
	| {
			readonly collapseExpandedBackingModel?: boolean;
			readonly groupId: string;
			readonly type: 'arm-group';
	  }
	| {
			readonly type: 'activate-drag';
	  }
	| {
			readonly preRemovalTargetIndex: number;
			readonly type: 'move-to';
	  }
	| {
			readonly type: 'commit';
	  }
	| {
			readonly type: 'cancel';
	  }
	| {
			readonly type: 'settle';
	  };
export type HorizontalGroupDragCommand =
	| {
			readonly activeTabId: string;
			readonly selectedTabIds: readonly string[];
			readonly type: 'set-selection';
	  }
	| {
			readonly collapsed: boolean;
			readonly groupId: string;
			readonly type: 'set-group-collapsed';
	  }
	| {
			readonly groupId: string;
			readonly preRemovalTargetIndex: number;
			readonly type: 'move-group';
	  }
	| {
			readonly type: 'restore';
	  };
interface HorizontalGroupDragTransitionResult {
	readonly commands: readonly HorizontalGroupDragCommand[];
	readonly state: HorizontalGroupDragState;
}
function cloneAuthoritative(
	state: Readonly<HorizontalGroupDragAuthoritativeState>,
): HorizontalGroupDragAuthoritativeState {
	return {
		tabs: state.tabs.map((tab) => ({
			...tab,
		})),
		groups: state.groups.map((group) => ({
			...group,
		})),
		activeTabId: state.activeTabId,
		selectedTabIds: [...state.selectedTabIds],
		selectionAnchorId: state.selectionAnchorId,
	};
}
function updateGroupCollapsed(
	state: Readonly<HorizontalGroupDragAuthoritativeState>,
	groupId: string,
	collapsed: boolean,
): HorizontalGroupDragAuthoritativeState {
	return {
		...state,
		groups: state.groups.map((group) =>
			group.id === groupId
				? {
						...group,
						collapsed,
					}
				: group,
		),
	};
}
function groupRange(
	tabs: readonly Readonly<HorizontalGroupDragTab>[],
	groupId: string,
): Readonly<{
	end: number;
	start: number;
}> | null {
	const start = tabs.findIndex((tab) => tab.group === groupId);
	if (start === -1) return null;
	let end = start + 1;
	while (tabs[end]?.group === groupId) end += 1;
	return {
		start,
		end,
	};
}
function isCollectionBoundary(
	state: Readonly<HorizontalGroupDragAuthoritativeState>,
	session: Readonly<GroupDragSession>,
	candidate: number,
): boolean {
	if (!Number.isInteger(candidate) || candidate < 0 || candidate > state.tabs.length) return false;
	const range = groupRange(state.tabs, session.groupId);
	if (range === null) return false;
	if (candidate > range.start && candidate <= range.end) return false;
	const firstUnpinned = state.tabs.findIndex((tab) => !tab.pinned);
	const unpinnedStart = firstUnpinned === -1 ? state.tabs.length : firstUnpinned;
	if (candidate < unpinnedStart) return false;
	const left = state.tabs[candidate - 1];
	const right = state.tabs[candidate];
	if (left && right && left.group !== null && left.group === right.group) return false;
	return true;
}
function withSelection(
	state: Readonly<HorizontalGroupDragAuthoritativeState>,
	selectedTabIds: readonly string[],
	activeTabId: string,
): HorizontalGroupDragAuthoritativeState {
	return {
		...state,
		activeTabId,
		selectedTabIds: [...selectedTabIds],
		selectionAnchorId: activeTabId,
	};
}

function unchanged(state: HorizontalGroupDragState): HorizontalGroupDragTransitionResult {
	return {
		state,
		commands: [],
	};
}
export function createHorizontalGroupDragState(
	authoritative: Readonly<HorizontalGroupDragAuthoritativeState>,
): HorizontalGroupDragState {
	return {
		authoritative: cloneAuthoritative(authoritative),
		drag: {
			kind: 'idle',
		},
	};
}
export function transitionHorizontalGroupDrag(
	current: HorizontalGroupDragState,
	event: Readonly<HorizontalGroupDragEvent>,
): HorizontalGroupDragTransitionResult {
	const state = current;
	if (event.type === 'arm-group') {
		if (state.drag.kind !== 'idle') return unchanged(state);
		const group = state.authoritative.groups.find((candidate) => candidate.id === event.groupId);
		const memberIds = state.authoritative.tabs
			.filter((tab) => tab.group === event.groupId)
			.map((tab) => tab.id);
		if (!group || memberIds.length === 0) return unchanged(state);
		const recordedActiveTabId =
			state.authoritative.activeTabId !== null &&
			memberIds.includes(state.authoritative.activeTabId)
				? state.authoritative.activeTabId
				: requireDefined(memberIds[0]);
		return {
			commands: [],
			state: {
				authoritative: state.authoritative,
				drag: {
					kind: 'armed',
					groupId: event.groupId,
					memberIds,
					recordedActiveTabId,
					original: cloneAuthoritative(state.authoritative),
					wasCollapsedBeforeDrag: group.collapsed,
					collapseExpandedBackingModel: event.collapseExpandedBackingModel ?? true,
					visualKind: group.collapsed ? 'collapsed-header' : 'expanded-group',
					targetIndex: null,
				},
			},
		};
	}
	if (event.type === 'activate-drag') {
		if (state.drag.kind !== 'armed') return unchanged(state);
		const drag = state.drag;
		const group = state.authoritative.groups.find((candidate) => candidate.id === drag.groupId);
		const currentMembers = state.authoritative.tabs
			.filter((tab) => tab.group === drag.groupId)
			.map((tab) => tab.id);
		if (!group || currentMembers.join('\u0000') !== drag.memberIds.join('\u0000')) {
			return unchanged(state);
		}
		let authoritative = withSelection(
			state.authoritative,
			drag.memberIds,
			drag.recordedActiveTabId,
		);
		const commands: HorizontalGroupDragCommand[] = [
			{
				type: 'set-selection',
				selectedTabIds: drag.memberIds,
				activeTabId: drag.recordedActiveTabId,
			},
		];
		if (!drag.wasCollapsedBeforeDrag && drag.collapseExpandedBackingModel) {
			authoritative = updateGroupCollapsed(authoritative, drag.groupId, true);
			commands.push({
				type: 'set-group-collapsed',
				groupId: drag.groupId,
				collapsed: true,
			});
		}
		return {
			commands,
			state: {
				authoritative,
				drag: {
					...drag,
					kind: 'dragging',
				},
			},
		};
	}
	if (event.type === 'move-to') {
		if (
			state.drag.kind !== 'dragging' ||
			!isCollectionBoundary(state.authoritative, state.drag, event.preRemovalTargetIndex)
		) {
			return unchanged(state);
		}
		const range = groupRange(state.authoritative.tabs, state.drag.groupId);
		if (range === null) return unchanged(state);
		const tabs = moveContiguousBlock(state.authoritative.tabs, {
			start: range.start,
			length: range.end - range.start,
			preRemovalTargetIndex: event.preRemovalTargetIndex,
		});
		return {
			commands: [
				{
					type: 'move-group',
					groupId: state.drag.groupId,
					preRemovalTargetIndex: event.preRemovalTargetIndex,
				},
			],
			state: {
				authoritative: {
					...state.authoritative,
					tabs,
				},
				drag: {
					...state.drag,
					targetIndex: event.preRemovalTargetIndex,
				},
			},
		};
	}
	if (event.type === 'commit') {
		if (state.drag.kind !== 'dragging') return unchanged(state);
		const drag = state.drag;
		let authoritative = updateGroupCollapsed(
			state.authoritative,
			drag.groupId,
			drag.wasCollapsedBeforeDrag,
		);
		authoritative = withSelection(
			authoritative,
			[drag.recordedActiveTabId],
			drag.recordedActiveTabId,
		);
		const commands: HorizontalGroupDragCommand[] = [];
		const currentGroup = state.authoritative.groups.find((group) => group.id === drag.groupId);
		if (currentGroup?.collapsed !== drag.wasCollapsedBeforeDrag) {
			commands.push({
				type: 'set-group-collapsed',
				groupId: drag.groupId,
				collapsed: drag.wasCollapsedBeforeDrag,
			});
		}
		commands.push({
			type: 'set-selection',
			selectedTabIds: [drag.recordedActiveTabId],
			activeTabId: drag.recordedActiveTabId,
		});
		return {
			commands,
			state: {
				authoritative,
				drag: {
					...drag,
					kind: 'settling',
				},
			},
		};
	}
	if (event.type === 'cancel') {
		if (state.drag.kind === 'idle' || state.drag.kind === 'settling') return unchanged(state);
		return {
			commands: [
				{
					type: 'restore',
				},
			],
			state: {
				authoritative: cloneAuthoritative(state.drag.original),
				drag: {
					kind: 'idle',
				},
			},
		};
	}
	if (event.type === 'settle') {
		if (state.drag.kind !== 'settling') return unchanged(state);
		return {
			commands: [],
			state: {
				authoritative: state.authoritative,
				drag: {
					kind: 'idle',
				},
			},
		};
	}
	return unchanged(state);
}
