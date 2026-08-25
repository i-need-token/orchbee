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
			readonly displayReadyPresentationIds: readonly string[];
			readonly displayedTabId: string;
			readonly epoch: number;
			readonly expectedPresentationIds: readonly string[];
			readonly kind: 'activating';
			readonly targetTabId: string;
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
			readonly targetTabId: string;
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
	if (event.targetTabId !== state.targetTabId) return state;

	const displayReadyPresentationIds = includeOnce(
		state.displayReadyPresentationIds,
		event.presentationId,
	);
	const activationReadyPresentationIds =
		event.readiness === 'activation-ready'
			? includeOnce(state.activationReadyPresentationIds, event.presentationId)
			: state.activationReadyPresentationIds;
	if (
		activationReadyPresentationIds === state.activationReadyPresentationIds &&
		displayReadyPresentationIds === state.displayReadyPresentationIds
	) {
		return state;
	}
	return derivePendingState({
		activationReadyPresentationIds,
		displayReadyPresentationIds,
		displayedTabId: state.displayedTabId,
		epoch: state.epoch,
		expectedPresentationIds: state.expectedPresentationIds,
		targetTabId: state.targetTabId,
	});
}

function observeTarget(
	state: TerminalTabPresentationHandoffState | null,
	event: Extract<TerminalTabPresentationHandoffEvent, { readonly type: 'target-observed' }>,
): TerminalTabPresentationHandoffState {
	const expectedPresentationIds = uniquePresentationIds(event.expectedPresentationIds);
	if (state === null) return settled(event.targetTabId, 0);
	const currentTargetTabId = state.kind === 'settled' ? state.displayedTabId : state.targetTabId;
	if (event.targetTabId === currentTargetTabId) {
		if (state.kind === 'settled') return state;
		const displayedTabId = event.outgoingAvailable ? state.displayedTabId : null;
		if (
			displayedTabId === state.displayedTabId &&
			samePresentationIdentitySet(expectedPresentationIds, state.expectedPresentationIds)
		) {
			return state;
		}
		return derivePendingState({
			activationReadyPresentationIds: state.activationReadyPresentationIds,
			displayReadyPresentationIds: state.displayReadyPresentationIds,
			displayedTabId,
			epoch: state.epoch,
			expectedPresentationIds,
			targetTabId: state.targetTabId,
		});
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

function derivePendingState({
	activationReadyPresentationIds,
	displayReadyPresentationIds,
	displayedTabId,
	epoch,
	expectedPresentationIds,
	targetTabId,
}: {
	readonly activationReadyPresentationIds: readonly string[];
	readonly displayReadyPresentationIds: readonly string[];
	readonly displayedTabId: string | null;
	readonly epoch: number;
	readonly expectedPresentationIds: readonly string[];
	readonly targetTabId: string;
}): TerminalTabPresentationHandoffState {
	if (containsEvery(activationReadyPresentationIds, expectedPresentationIds)) {
		return settled(targetTabId, epoch);
	}
	if (containsEvery(displayReadyPresentationIds, expectedPresentationIds)) {
		return Object.freeze({
			activationReadyPresentationIds,
			displayReadyPresentationIds,
			displayedTabId: targetTabId,
			epoch,
			expectedPresentationIds,
			kind: 'activating',
			targetTabId,
		});
	}
	return Object.freeze({
		activationReadyPresentationIds,
		displayReadyPresentationIds,
		displayedTabId,
		epoch,
		expectedPresentationIds,
		kind: 'preparing',
		targetTabId,
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

function samePresentationIdentitySet(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && containsEvery(left, right);
}
