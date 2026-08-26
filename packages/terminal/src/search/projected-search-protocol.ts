import type { TerminalBufferRow } from '../buffer-rows';
import type {
	TerminalDirectionalSearchOptions,
	TerminalSearchMatch,
	TerminalSearchOptions,
} from './terminal-search';

export const PROJECTED_SEARCH_SOURCE_BYTE_LIMIT = 16 * 1024 * 1024;

export type ProjectedSearchWorkerRequest =
	| {
			readonly generation: string;
			readonly kind: 'start';
			readonly navigation: TerminalDirectionalSearchOptions;
			readonly options: TerminalSearchOptions;
			readonly query: string;
			readonly revision: string;
			readonly searchId: number;
	  }
	| {
			readonly final: boolean;
			readonly generation: string;
			readonly kind: 'page';
			readonly revision: string;
			readonly rows: readonly TerminalBufferRow[];
			readonly searchId: number;
	  };

export type ProjectedSearchWorkerResponse =
	| {
			readonly activeMatch: TerminalSearchMatch | null;
			readonly generation: string;
			readonly kind: 'result';
			readonly matches: readonly TerminalSearchMatch[];
			readonly revision: string;
			readonly searchId: number;
	  }
	| {
			readonly generation: string;
			readonly kind: 'progress';
			readonly nextRow: string;
			readonly revision: string;
			readonly searchId: number;
	  }
	| {
			readonly code: 'invalid-sequence' | 'source-too-large' | 'worker-failed';
			readonly kind: 'error';
			readonly message: string;
			readonly searchId: number;
	  };

export interface ProjectedSearchWorkerLike {
	addEventListener(type: 'error', listener: (event: ErrorEvent) => void): void;
	addEventListener(
		type: 'message',
		listener: (event: MessageEvent<ProjectedSearchWorkerResponse>) => void,
	): void;
	postMessage(message: ProjectedSearchWorkerRequest): void;
	terminate(): void;
}

export type ProjectedSearchWorkerFactory = () => ProjectedSearchWorkerLike;
