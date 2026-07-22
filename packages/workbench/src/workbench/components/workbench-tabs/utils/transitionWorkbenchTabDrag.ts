import { reduceWorkbenchTabs } from '../../../../workbench-tabs';
import type {
	WorkbenchTabDragCommand,
	WorkbenchTabDragEvent,
	WorkbenchTabDragState,
	WorkbenchTabDragTransition,
} from '../types/workbench-tab-drag';
import {
	resolveWorkbenchTabDragProjection,
	resolveWorkbenchTabGroupDragProjection,
} from './workbench-tab-drag-projection';

const DRAG_START_DISTANCE = 10;
const HORIZONTAL_REORDER_THRESHOLD = 16;
const STANDARD_TAB_WIDTH = 200;

function getReorderThreshold(draggedWidth: number): number {
	return Math.max(
		1,
		Math.round((draggedWidth / STANDARD_TAB_WIDTH) * HORIZONTAL_REORDER_THRESHOLD),
	);
}

function unchanged(state: WorkbenchTabDragState): WorkbenchTabDragTransition {
	return { commands: [], state };
}

function toIdle(commands: readonly WorkbenchTabDragCommand[] = []): WorkbenchTabDragTransition {
	return { commands, state: { kind: 'idle' } };
}

function toSettling(
	previewState: Extract<
		WorkbenchTabDragState,
		{ readonly kind: 'dragging-group' | 'dragging-tab' }
	>['previewState'],
	commands: readonly WorkbenchTabDragCommand[],
): WorkbenchTabDragTransition {
	return { commands, state: { kind: 'settling', previewState } };
}

export function transitionWorkbenchTabDrag(
	state: WorkbenchTabDragState,
	event: WorkbenchTabDragEvent,
): WorkbenchTabDragTransition {
	if (event.kind === 'stale-input') return state.kind === 'idle' ? unchanged(state) : toIdle();
	if (event.kind === 'acknowledge-commit')
		return state.kind === 'settling' ? toIdle() : unchanged(state);
	if (state.kind === 'settling') {
		return event.kind === 'cancel' ? toIdle() : unchanged(state);
	}
	if (event.kind === 'cancel') {
		if (state.kind === 'idle') return unchanged(state);
		if (event.pointerId !== undefined && event.pointerId !== state.pointerId)
			return unchanged(state);
		return toIdle();
	}
	if (event.kind === 'arm-tab') {
		if (state.kind !== 'idle') return unchanged(state);
		if (!event.state.tabs.some((tab) => tab.id === event.draggedTabId)) return unchanged(state);
		return {
			commands: [],
			state: {
				draggedTabId: event.draggedTabId,
				draggedWidth: event.draggedWidth,
				grabOffset: event.grabOffset,
				kind: 'armed-tab',
				layout: event.layout,
				pointerId: event.pointerId,
				pointerX: event.pointerX,
				pointerY: event.pointerY,
				sourceLeft: event.sourceLeft,
				sourcePointerX: event.pointerX,
				sourcePointerY: event.pointerY,
				sourceState: event.state,
			},
		};
	}
	if (event.kind === 'arm-group') {
		if (state.kind !== 'idle') return unchanged(state);
		if (!event.state.tabs.some((tab) => tab.groupId === event.draggedGroupId)) {
			return unchanged(state);
		}
		return {
			commands: [],
			state: {
				draggedGroupId: event.draggedGroupId,
				draggedWidth: event.draggedWidth,
				grabOffset: event.grabOffset,
				kind: 'armed-group',
				layout: event.layout,
				pointerId: event.pointerId,
				pointerX: event.pointerX,
				pointerY: event.pointerY,
				sourceLeft: event.sourceLeft,
				sourcePointerX: event.pointerX,
				sourcePointerY: event.pointerY,
				sourceState: event.state,
			},
		};
	}
	if (state.kind === 'idle') return unchanged(state);
	if (event.pointerId !== state.pointerId) return unchanged(state);
	if (event.kind === 'move' && !event.primaryButtonPressed) return toIdle();
	if (event.kind === 'release') {
		if (state.kind === 'armed-tab' || state.kind === 'armed-group') return toIdle();
		if (state.kind === 'dragging-tab') {
			const sourceTab = state.sourceState.tabs.find((tab) => tab.id === state.draggedTabId);
			const previewTab = state.previewState.tabs.find((tab) => tab.id === state.draggedTabId);
			const targetIndex = state.previewState.tabs.findIndex((tab) => tab.id === state.draggedTabId);
			if (
				sourceTab === undefined ||
				previewTab === undefined ||
				(state.sourceState.tabs[targetIndex]?.id === previewTab.id &&
					sourceTab.groupId === previewTab.groupId)
			) {
				return toIdle();
			}
			return toSettling(state.previewState, [
				{
					groupId: previewTab.groupId,
					kind: 'commit-tab-move',
					tabId: state.draggedTabId,
					targetIndex,
				},
			]);
		}
		const sourceIndex = state.sourceState.tabs.findIndex(
			(tab) => tab.groupId === state.draggedGroupId,
		);
		const targetIndex = state.previewState.tabs.findIndex(
			(tab) => tab.groupId === state.draggedGroupId,
		);
		return sourceIndex === targetIndex
			? toIdle()
			: toSettling(state.previewState, [
					{
						groupId: state.draggedGroupId,
						kind: 'commit-tab-group-move',
						targetIndex,
					},
				]);
	}
	if (event.kind !== 'move') return unchanged(state);
	const pointerDistance = Math.hypot(
		event.pointerX - state.sourcePointerX,
		event.pointerY - state.sourcePointerY,
	);
	if (
		(state.kind === 'armed-tab' || state.kind === 'armed-group') &&
		pointerDistance < DRAG_START_DISTANCE
	) {
		return unchanged(state);
	}

	if (state.kind === 'armed-tab' || state.kind === 'dragging-tab') {
		const previewState = state.kind === 'armed-tab' ? state.sourceState : state.previewState;
		const draggedCenter = event.pointerX - state.grabOffset + state.draggedWidth / 2;
		const projection = resolveWorkbenchTabDragProjection(
			previewState,
			state.layout,
			state.draggedTabId,
			draggedCenter,
			state.draggedWidth,
		);
		let nextPreviewState =
			projection === undefined
				? previewState
				: reduceWorkbenchTabs(previewState, {
						groupId: projection.groupId,
						kind: 'move-tab',
						tabId: state.draggedTabId,
						targetIndex: projection.targetIndex,
					});
		let lastAcceptedPointerX =
			state.kind === 'dragging-tab' ? state.lastAcceptedPointerX : state.sourcePointerX;
		if (
			state.kind === 'dragging-tab' &&
			nextPreviewState !== previewState &&
			Math.abs(event.pointerX - state.lastAcceptedPointerX) <=
				getReorderThreshold(state.draggedWidth)
		) {
			nextPreviewState = previewState;
		} else if (nextPreviewState !== previewState) {
			lastAcceptedPointerX = event.pointerX;
		}
		const commands: readonly WorkbenchTabDragCommand[] =
			state.kind === 'armed-tab' ? [{ kind: 'activate-tab', tabId: state.draggedTabId }] : [];
		return {
			commands,
			state: {
				...state,
				kind: 'dragging-tab',
				lastAcceptedPointerX,
				pointerX: event.pointerX,
				pointerY: event.pointerY,
				previewState: nextPreviewState,
			},
		};
	}

	const previewState = state.kind === 'armed-group' ? state.sourceState : state.previewState;
	const draggedLeft = event.pointerX - state.grabOffset;
	const targetIndex = resolveWorkbenchTabGroupDragProjection(
		previewState,
		state.layout,
		state.draggedGroupId,
		draggedLeft,
	);
	let nextPreviewState =
		targetIndex === undefined
			? previewState
			: reduceWorkbenchTabs(previewState, {
					groupId: state.draggedGroupId,
					kind: 'move-tab-group',
					targetIndex,
				});
	let lastAcceptedPointerX =
		state.kind === 'dragging-group' ? state.lastAcceptedPointerX : state.sourcePointerX;
	if (
		state.kind === 'dragging-group' &&
		nextPreviewState !== previewState &&
		Math.abs(event.pointerX - state.lastAcceptedPointerX) <= getReorderThreshold(state.draggedWidth)
	) {
		nextPreviewState = previewState;
	} else if (nextPreviewState !== previewState) {
		lastAcceptedPointerX = event.pointerX;
	}
	const commands: readonly WorkbenchTabDragCommand[] =
		state.kind === 'armed-group'
			? [{ groupId: state.draggedGroupId, kind: 'activate-tab-group' }]
			: [];
	return {
		commands,
		state: {
			...state,
			kind: 'dragging-group',
			lastAcceptedPointerX,
			pointerX: event.pointerX,
			pointerY: event.pointerY,
			previewState: nextPreviewState,
		},
	};
}
