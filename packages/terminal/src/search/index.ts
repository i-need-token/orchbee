export {
	findDirectionalTerminalMatch,
	findTerminalBufferMatches,
	findTerminalMatches,
	resolveTerminalSearchResultLimit,
	searchMatchContainsCell,
	TERMINAL_SEARCH_DEBOUNCE_MS,
	TERMINAL_SEARCH_RESULT_LIMIT,
	validateTerminalSearchQuery,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
	type TerminalSearchResultChangeEvent,
	type TerminalDirectionalSearchOptions,
} from './terminal-search';
export {
	createProjectedSearchWorker,
	ProjectedSearchController,
	type ProjectedSearchControllerOptions,
	type ProjectedTerminalSearchEvent,
} from './projected-search-controller';
export type {
	ProjectedSearchWorkerFactory,
	ProjectedSearchWorkerLike,
} from './projected-search-protocol';
