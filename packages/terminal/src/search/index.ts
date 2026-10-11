export {
	findTerminalBufferMatches,
	searchMatchContainsCell,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from './terminal-search';
export {
	createProjectedSearchWorker,
	ProjectedSearchController,
	type ProjectedTerminalSearchEvent,
} from './projected-search-controller';
export type {
	ProjectedSearchWorkerFactory,
	ProjectedSearchWorkerLike,
} from './projected-search-protocol';
