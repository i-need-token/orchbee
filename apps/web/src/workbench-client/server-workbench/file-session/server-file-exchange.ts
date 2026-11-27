import type { FileError, FileResult } from '../../../file-session';
import {
	type ServerConnection,
	ServerConnectionEndedError,
	ServerExchangeTimeoutError,
	ServerStreamRefusedError,
} from '../../server-connection';

/** What the File Bar adapters use of their Server connection. */
export type ServerFileConnection = Pick<
	ServerConnection,
	'closeLane' | 'exchange' | 'onEnded' | 'openLane' | 'subscribe'
>;

/** The paths one request names at most, by the backend's bound. */
const PATHS_PER_REQUEST = 256;
/** Room for a request's paths in its 16 KiB record, after its other members. */
const PATH_BYTES_PER_REQUEST = 16_384 - 512;

const pathEncoder = new TextEncoder();

export const CANCELLED: FileResult<never> = { error: { code: 'cancelled' }, kind: 'error' };

export function fileFailure(code: FileError['code']): FileResult<never> {
	return { error: { code }, kind: 'error' };
}

/** What a request that got no Result settles with. */
export function unansweredError(error: unknown): FileError {
	if (error instanceof ServerConnectionEndedError) return { code: 'remote-disconnected' };
	if (error instanceof ServerExchangeTimeoutError) return { code: 'timeout' };
	if (!(error instanceof ServerStreamRefusedError)) return { code: 'failed' };
	switch (error.reason) {
		case 'cancelled':
			return { code: 'cancelled' };
		case 'invalid-request':
			return { code: 'invalid-request' };
		// The machine lacks the operation, has no room for it now, or is stopping.
		case 'capacity':
		case 'stopping':
		case 'unsupported':
			return { code: 'unavailable' };
		case 'complete':
		case 'outcome-unknown':
			return { code: 'failed' };
	}
}

/** Settles `cancelled` once `signal` aborts, and otherwise as `answer` does. */
export function untilAborted<Value>(
	answer: Promise<FileResult<Value>>,
	signal: AbortSignal,
): Promise<FileResult<Value>> {
	if (signal.aborted) return Promise.resolve(CANCELLED);
	return new Promise((resolve) => {
		const abort = (): void => resolve(CANCELLED);
		signal.addEventListener('abort', abort, { once: true });
		void answer.then((result) => {
			signal.removeEventListener('abort', abort);
			resolve(result);
		});
	});
}

/**
 * Splits paths into requests within the path count and record size a request may carry; most
 * requests take 256 paths, `git.check-ignore` 1,000.
 */
export function pathBatches(
	paths: readonly string[],
	pathsPerRequest = PATHS_PER_REQUEST,
): readonly (readonly string[])[] {
	const batches: string[][] = [];
	let batch: string[] = [];
	let bytes = 0;
	for (const path of paths) {
		const pathBytes = pathEncoder.encode(JSON.stringify(path)).byteLength + 1;
		if (
			batch.length > 0 &&
			(batch.length === pathsPerRequest || bytes + pathBytes > PATH_BYTES_PER_REQUEST)
		) {
			batches.push(batch);
			batch = [];
			bytes = 0;
		}
		batch.push(path);
		bytes += pathBytes;
	}
	if (batch.length > 0) batches.push(batch);
	return batches;
}
