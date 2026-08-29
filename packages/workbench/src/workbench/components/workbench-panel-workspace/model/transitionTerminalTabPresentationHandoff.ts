import type { WorkbenchTerminalPresentationReadiness } from '../../../types/workbench-terminal-surface';

type TerminalTabPresentationReadiness = WorkbenchTerminalPresentationReadiness;

export type TerminalTabPresentationHandoffState =
	| {
			readonly displayedRevision: string | null;
			readonly displayedTabId: string | null;
			readonly epoch: number;
			readonly kind: 'settled';
	  }
	| {
			readonly activationReadyPresentationIds: readonly string[];
			readonly displayedRevision: string | null;
			readonly displayedTabId: string | null;
			readonly displayReadyPresentationIds: readonly string[];
			readonly epoch: number;
			readonly expectedPresentationIds: readonly string[];
			readonly kind: 'preparing';
			readonly targetRevision: string;
			readonly targetTabId: string;
	  }
	| {
			readonly activationReadyPresentationIds: readonly string[];
			readonly displayedRevision: string;
			readonly displayedTabId: string;
			readonly displayReadyPresentationIds: readonly string[];
			readonly epoch: number;
			readonly expectedPresentationIds: readonly string[];
			readonly kind: 'activating';
			readonly targetRevision: string;
			readonly targetTabId: string;
	  };

export type TerminalTabPresentationHandoffEvent =
	| {
			readonly expectedPresentationIds: readonly string[];
			readonly outgoingAvailable: boolean;
			readonly targetRevision: string | null;
			readonly targetTabId: string | null;
			readonly type: 'target-observed';
	  }
	| {
			readonly epoch: number;
			readonly presentationId: string;
			readonly readiness: TerminalTabPresentationReadiness;
			readonly targetRevision: string;
			readonly targetTabId: string;
			readonly type: 'presentation-ready';
	  };

/** Owns the visual and activation barriers for one mounted Workbench presentation scope. */
export function transitionTerminalTabPresentationHandoff(
	state: TerminalTabPresentationHandoffState | null,
	event: TerminalTabPresentationHandoffEvent,
): TerminalTabPresentationHandoffState {
	if (event.type === 'target-observed') return observeTarget(state, event);
	if (state === null) return settled(null, null, 0);
	if (state.kind === 'settled' || event.epoch !== state.epoch) return state;
	if (event.targetTabId !== state.targetTabId || event.targetRevision !== state.targetRevision) {
		return state;
	}

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
		displayedRevision: state.displayedRevision,
		displayedTabId: state.displayedTabId,
		displayReadyPresentationIds,
		epoch: state.epoch,
		expectedPresentationIds: state.expectedPresentationIds,
		targetRevision: state.targetRevision,
		targetTabId: state.targetTabId,
	});
}

function observeTarget(
	state: TerminalTabPresentationHandoffState | null,
	event: Extract<TerminalTabPresentationHandoffEvent, { readonly type: 'target-observed' }>,
): TerminalTabPresentationHandoffState {
	const expectedPresentationIds = uniquePresentationIds(event.expectedPresentationIds);
	if (state === null) return settled(event.targetTabId, event.targetRevision, 0);
	const currentTargetTabId = state.kind === 'settled' ? state.displayedTabId : state.targetTabId;
	const currentTargetRevision =
		state.kind === 'settled' ? state.displayedRevision : state.targetRevision;
	if (event.targetTabId === currentTargetTabId && event.targetRevision === currentTargetRevision) {
		if (state.kind === 'settled') return state;
		const displayedTabId = event.outgoingAvailable ? state.displayedTabId : null;
		const displayedRevision = event.outgoingAvailable ? state.displayedRevision : null;
		if (
			displayedTabId === state.displayedTabId &&
			displayedRevision === state.displayedRevision &&
			samePresentationIdentitySet(expectedPresentationIds, state.expectedPresentationIds)
		) {
			return state;
		}
		return derivePendingState({
			activationReadyPresentationIds: state.activationReadyPresentationIds,
			displayedRevision,
			displayedTabId,
			displayReadyPresentationIds: state.displayReadyPresentationIds,
			epoch: state.epoch,
			expectedPresentationIds,
			targetRevision: state.targetRevision,
			targetTabId: state.targetTabId,
		});
	}

	const epoch = state.epoch + 1;
	if (
		event.targetTabId === null ||
		event.targetRevision === null ||
		expectedPresentationIds.length === 0
	) {
		return settled(event.targetTabId, event.targetRevision, epoch);
	}
	return Object.freeze({
		activationReadyPresentationIds: Object.freeze([]),
		displayedRevision: event.outgoingAvailable ? state.displayedRevision : null,
		displayedTabId: event.outgoingAvailable ? state.displayedTabId : null,
		displayReadyPresentationIds: Object.freeze([]),
		epoch,
		expectedPresentationIds,
		kind: 'preparing',
		targetRevision: event.targetRevision,
		targetTabId: event.targetTabId,
	});
}

function derivePendingState({
	activationReadyPresentationIds,
	displayedRevision,
	displayedTabId,
	displayReadyPresentationIds,
	epoch,
	expectedPresentationIds,
	targetRevision,
	targetTabId,
}: {
	readonly activationReadyPresentationIds: readonly string[];
	readonly displayedRevision: string | null;
	readonly displayedTabId: string | null;
	readonly displayReadyPresentationIds: readonly string[];
	readonly epoch: number;
	readonly expectedPresentationIds: readonly string[];
	readonly targetRevision: string;
	readonly targetTabId: string;
}): TerminalTabPresentationHandoffState {
	if (containsEvery(activationReadyPresentationIds, expectedPresentationIds)) {
		return settled(targetTabId, targetRevision, epoch);
	}
	if (containsEvery(displayReadyPresentationIds, expectedPresentationIds)) {
		return Object.freeze({
			activationReadyPresentationIds,
			displayedRevision: targetRevision,
			displayedTabId: targetTabId,
			displayReadyPresentationIds,
			epoch,
			expectedPresentationIds,
			kind: 'activating',
			targetRevision,
			targetTabId,
		});
	}
	return Object.freeze({
		activationReadyPresentationIds,
		displayedRevision,
		displayedTabId,
		displayReadyPresentationIds,
		epoch,
		expectedPresentationIds,
		kind: 'preparing',
		targetRevision,
		targetTabId,
	});
}

function settled(
	displayedTabId: string | null,
	displayedRevision: string | null,
	epoch: number,
): TerminalTabPresentationHandoffState {
	return Object.freeze({ displayedRevision, displayedTabId, epoch, kind: 'settled' });
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
