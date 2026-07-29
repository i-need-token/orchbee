import type { WorkbenchSearchFilter } from '../../../types/workbench-search';

interface WorkbenchSearchSnapshot {
	readonly filter: WorkbenchSearchFilter;
	readonly highlightedResultId: string | null;
	readonly query: string;
	readonly queryRevision: number;
	readonly scrollTop: number;
}

export type WorkbenchSearchState =
	| {
			readonly kind: 'closed';
			readonly nextSessionId: number;
	  }
	| ({
			readonly kind: 'browsing';
			readonly nextSessionId: number;
			readonly sessionId: number;
	  } & WorkbenchSearchSnapshot)
	| {
			readonly actionResultId: string;
			readonly highlightedResultId: string | null;
			readonly kind: 'picking-argument';
			readonly nextSessionId: number;
			readonly parent: WorkbenchSearchSnapshot;
			readonly query: string;
			readonly queryRevision: number;
			readonly scrollTop: number;
			readonly sessionId: number;
	  };

export type WorkbenchSearchEvent =
	| { readonly filter: WorkbenchSearchFilter; readonly type: 'open' }
	| { readonly type: 'close' }
	| { readonly filter: WorkbenchSearchFilter; readonly type: 'filter-changed' }
	| { readonly query: string; readonly type: 'query-changed' }
	| {
			readonly enabledResultIds: readonly string[];
			readonly resultIds: readonly string[];
			readonly type: 'results-changed';
	  }
	| { readonly direction: -1 | 1; readonly resultIds: readonly string[]; readonly type: 'move' }
	| { readonly resultId: string; readonly type: 'highlight' }
	| { readonly scrollTop: number; readonly type: 'scroll-changed' }
	| { readonly actionResultId: string; readonly type: 'start-argument-picker' }
	| { readonly type: 'back' };

export function createWorkbenchSearchState(): WorkbenchSearchState {
	return { kind: 'closed', nextSessionId: 1 };
}

export function transitionWorkbenchSearch(
	state: WorkbenchSearchState,
	event: WorkbenchSearchEvent,
): WorkbenchSearchState {
	switch (event.type) {
		case 'open':
			return {
				filter: event.filter,
				highlightedResultId: null,
				kind: 'browsing',
				nextSessionId: state.nextSessionId + 1,
				query: '',
				queryRevision: 0,
				scrollTop: 0,
				sessionId: state.nextSessionId,
			};
		case 'close':
			return { kind: 'closed', nextSessionId: state.nextSessionId };
		case 'filter-changed':
			return state.kind === 'browsing' && event.filter !== state.filter
				? {
						...state,
						filter: event.filter,
						queryRevision: state.queryRevision + 1,
						scrollTop: 0,
					}
				: state;
		case 'query-changed':
			if (state.kind === 'closed' || event.query === state.query) {
				return state;
			}
			return {
				...state,
				query: event.query,
				queryRevision: state.queryRevision + 1,
				scrollTop: 0,
			};
		case 'results-changed':
			if (state.kind === 'closed') {
				return state;
			}
			return {
				...state,
				highlightedResultId: resolveHighlight(
					state.highlightedResultId,
					event.resultIds,
					event.enabledResultIds,
				),
			};
		case 'move':
			if (state.kind === 'closed') {
				return state;
			}
			return {
				...state,
				highlightedResultId: moveHighlight(
					state.highlightedResultId,
					event.resultIds,
					event.direction,
				),
			};
		case 'highlight':
			return state.kind === 'closed' ? state : { ...state, highlightedResultId: event.resultId };
		case 'scroll-changed':
			return state.kind === 'closed' || event.scrollTop === state.scrollTop
				? state
				: { ...state, scrollTop: Math.max(0, event.scrollTop) };
		case 'start-argument-picker':
			return state.kind !== 'browsing'
				? state
				: {
						actionResultId: event.actionResultId,
						highlightedResultId: null,
						kind: 'picking-argument',
						nextSessionId: state.nextSessionId,
						parent: {
							filter: state.filter,
							highlightedResultId: state.highlightedResultId,
							query: state.query,
							queryRevision: state.queryRevision,
							scrollTop: state.scrollTop,
						},
						query: '',
						queryRevision: state.queryRevision + 1,
						scrollTop: 0,
						sessionId: state.sessionId,
					};
		case 'back':
			return state.kind !== 'picking-argument'
				? state
				: {
						...state.parent,
						kind: 'browsing',
						nextSessionId: state.nextSessionId,
						sessionId: state.sessionId,
					};
	}
}

function resolveHighlight(
	currentId: string | null,
	resultIds: readonly string[],
	enabledResultIds: readonly string[],
): string | null {
	if (currentId !== null && resultIds.includes(currentId)) {
		return currentId;
	}
	return enabledResultIds[0] ?? resultIds[0] ?? null;
}

function moveHighlight(
	currentId: string | null,
	resultIds: readonly string[],
	direction: -1 | 1,
): string | null {
	if (resultIds.length === 0) {
		return null;
	}
	const currentIndex = currentId === null ? -1 : resultIds.indexOf(currentId);
	if (currentIndex === -1) {
		return direction === 1 ? (resultIds[0] ?? null) : (resultIds.at(-1) ?? null);
	}
	const nextIndex = (currentIndex + direction + resultIds.length) % resultIds.length;
	return resultIds[nextIndex] ?? null;
}
