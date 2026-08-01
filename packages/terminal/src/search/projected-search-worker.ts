import type {
	ProjectedSearchWorkerRequest,
	ProjectedSearchWorkerResponse,
} from './projected-search-protocol';
import { ProjectedSearchAccumulator } from './ProjectedSearchAccumulator';

const MAX_SEARCH_SOURCE_BYTES = 16 * 1024 * 1024;
const textEncoder = new TextEncoder();

interface ActiveSearch {
	readonly accumulator: ProjectedSearchAccumulator;
	readonly generation: string;
	readonly revision: string;
	readonly searchId: number;
	nextRow: bigint;
	sourceBytes: number;
}

let active: ActiveSearch | undefined;

globalThis.onmessage = (event: MessageEvent<ProjectedSearchWorkerRequest>): void => {
	handleProjectedSearchWorkerMessage(event.data, (message) => globalThis.postMessage(message));
};

function handleProjectedSearchWorkerMessage(
	message: ProjectedSearchWorkerRequest,
	postMessage: (message: ProjectedSearchWorkerResponse) => void,
): void {
	if (message.kind === 'start') {
		active = {
			accumulator: new ProjectedSearchAccumulator(message.query, message.options),
			generation: message.generation,
			revision: message.revision,
			searchId: message.searchId,
			nextRow: 0n,
			sourceBytes: 0,
		};
		return;
	}
	const search = active;
	if (
		search === undefined ||
		message.searchId !== search.searchId ||
		message.revision !== search.revision ||
		message.generation !== search.generation
	) {
		postError(
			postMessage,
			message.searchId,
			'invalid-sequence',
			'Search page does not match the active scan.',
		);
		return;
	}
	for (const row of message.rows) {
		if (row.index !== search.nextRow) {
			active = undefined;
			postError(
				postMessage,
				message.searchId,
				'invalid-sequence',
				'Search rows are not contiguous.',
			);
			return;
		}
		search.sourceBytes += textEncoder.encode(row.text).byteLength;
		if (search.sourceBytes > MAX_SEARCH_SOURCE_BYTES) {
			active = undefined;
			postError(
				postMessage,
				message.searchId,
				'source-too-large',
				'Search source exceeds its bounded limit.',
			);
			return;
		}
		search.nextRow += 1n;
	}
	try {
		const matches = search.accumulator.push(message.rows, message.final);
		if (matches === null) return;
		postMessage({
			generation: search.generation,
			kind: 'result',
			matches,
			revision: search.revision,
			searchId: search.searchId,
		});
	} catch (error) {
		postError(
			postMessage,
			search.searchId,
			'worker-failed',
			error instanceof Error ? error.message : 'Terminal search failed.',
		);
	}
	active = undefined;
}

function postError(
	postMessage: (message: ProjectedSearchWorkerResponse) => void,
	searchId: number,
	code: Extract<ProjectedSearchWorkerResponse, { kind: 'error' }>['code'],
	message: string,
): void {
	postMessage({ code, kind: 'error', message, searchId });
}
