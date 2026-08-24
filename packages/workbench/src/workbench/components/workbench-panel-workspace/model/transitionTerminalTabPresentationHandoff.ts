import type { WorkbenchTerminalPresentationReadiness } from '../../../types/workbench-terminal-surface';

type TerminalTabPresentationReadiness = WorkbenchTerminalPresentationReadiness;

export type TerminalTabPresentationHandoffState =
	| {
			readonly displayedTabId: string | null;
			readonly epoch: number;
			readonly kind: 'settled';
	  }
	| {
			readonly activationReadyPresentationIds: readonly string[];
			readonly displayReadyPresentationIds: readonly string[];
			readonly displayedTabId: string | null;
			readonly epoch: number;
			readonly expectedPresentationIds: readonly string[];
			readonly kind: 'preparing';
			readonly targetTabId: string;
	  }
	| {
			readonly activationReadyPresentationIds: readonly string[];
			readonly displayedTabId: string;
			readonly epoch: number;
			readonly expectedPresentationIds: readonly string[];
			readonly kind: 'activating';
	  };

export type TerminalTabPresentationHandoffEvent =
	| {
			readonly expectedPresentationIds: readonly string[];
			readonly outgoingAvailable: boolean;
			readonly targetTabId: string | null;
			readonly type: 'target-observed';
	  }
	| {
			readonly epoch: number;
			readonly presentationId: string;
			readonly readiness: TerminalTabPresentationReadiness;
			readonly type: 'presentation-ready';
	  };

/** Owns the visual and activation barriers for one mounted Workbench's active Tab. */
export function transitionTerminalTabPresentationHandoff(
	state: TerminalTabPresentationHandoffState | null,
	event: TerminalTabPresentationHandoffEvent,
): TerminalTabPresentationHandoffState {
	if (event.type === 'target-observed') return observeTarget(state, event);
	if (state === null) return settled(null, 0);
	if (state.kind === 'settled' || event.epoch !== state.epoch) return state;
	if (!state.expectedPresentationIds.includes(event.presentationId)) return state;

	const activationReadyPresentationIds =
		event.readiness === 'activation-ready'
			? includeOnce(state.activationReadyPresentationIds, event.presentationId)
			: state.activationReadyPresentationIds;
	if (state.kind === 'activating') {
		if (activationReadyPresentationIds === state.activationReadyPresentationIds) return state;
		return containsEvery(activationReadyPresentationIds, state.expectedPresentationIds)
			? settled(state.displayedTabId, state.epoch)
			: Object.freeze({ ...state, activationReadyPresentationIds });
	}

	const displayReadyPresentationIds = includeOnce(
		state.displayReadyPresentationIds,
		event.presentationId,
	);
	if (
		activationReadyPresentationIds === state.activationReadyPresentationIds &&
		displayReadyPresentationIds === state.displayReadyPresentationIds
	) {
		return state;
	}
	if (!containsEvery(displayReadyPresentationIds, state.expectedPresentationIds)) {
		return Object.freeze({
			...state,
			activationReadyPresentationIds,
			displayReadyPresentationIds,
		});
	}
	if (containsEvery(activationReadyPresentationIds, state.expectedPresentationIds)) {
		return settled(state.targetTabId, state.epoch);
	}
	return Object.freeze({
		activationReadyPresentationIds,
		displayedTabId: state.targetTabId,
		epoch: state.epoch,
		expectedPresentationIds: state.expectedPresentationIds,
		kind: 'activating',
	});
}

function observeTarget(
	state: TerminalTabPresentationHandoffState | null,
	event: Extract<TerminalTabPresentationHandoffEvent, { readonly type: 'target-observed' }>,
): TerminalTabPresentationHandoffState {
	const expectedPresentationIds = uniquePresentationIds(event.expectedPresentationIds);
	if (state === null) return settled(event.targetTabId, 0);
	const currentTargetTabId = state.kind === 'preparing' ? state.targetTabId : state.displayedTabId;
	if (event.targetTabId === currentTargetTabId) {
		if (
			state.kind === 'settled' ||
			samePresentationIds(expectedPresentationIds, state.expectedPresentationIds)
		) {
			return state;
		}
	}
	const epoch = state.epoch + 1;
	if (event.targetTabId === null || expectedPresentationIds.length === 0) {
		return settled(event.targetTabId, epoch);
	}
	const displayedTabId = event.outgoingAvailable ? state.displayedTabId : null;
	return Object.freeze({
		activationReadyPresentationIds: Object.freeze([]),
		displayReadyPresentationIds: Object.freeze([]),
		displayedTabId,
		epoch,
		expectedPresentationIds,
		kind: 'preparing',
		targetTabId: event.targetTabId,
	});
}

function settled(
	displayedTabId: string | null,
	epoch: number,
): TerminalTabPresentationHandoffState {
	return Object.freeze({ displayedTabId, epoch, kind: 'settled' });
}

function uniquePresentationIds(presentationIds: readonly string[]): readonly string[] {
	const unique = new Set<string>();
	for (const presentationId of presentationIds) unique.add(presentationId);
	return Object.freeze([...unique]);
}

function includeOnce(
	presentationIds: readonly string[],
	presentationId: string,
): readonly string[] {
	return presentationIds.includes(presentationId)
		? presentationIds
		: Object.freeze([...presentationIds, presentationId]);
}

function containsEvery(actual: readonly string[], expected: readonly string[]): boolean {
	return expected.every((presentationId) => actual.includes(presentationId));
}

function samePresentationIds(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}
