import { execFile } from 'node:child_process';

import { z } from 'zod';

/** A status read answers from the Server or its activation record; this bounds one that hangs. */
const COMMAND_DEADLINE_MILLISECONDS = 10_000;

const ServerStopDocumentSchema = z.looseObject({
	result: z.looseObject({ outcome: z.string() }),
	schema: z.literal('orchbee.server.stop'),
});

/**
 * What `server stop-status` says of one stop: `stopped` only once its receipt proves completion,
 * `failed`, `busy`, and `unmanaged` as the Server reports them, and `undecided` for a stop still
 * running, a Server between closing and writing its receipt, or an answer that cannot be read.
 */
export type ServerStopStatus = 'busy' | 'failed' | 'stopped' | 'undecided' | 'unmanaged';

/** Reads the stop recorded under `stopId` with `server stop-status`, which never starts a Session. */
export function readServerStopStatus(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
	stopId: string,
): Promise<ServerStopStatus> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'server', 'stop-status', stopId, '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			// Every outcome writes its document to standard output, whatever the exit code says.
			(_error, stdout) => resolve(interpretServerStopStatus(stdout)),
		);
	});
}

function interpretServerStopStatus(stdout: string): ServerStopStatus {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return 'undecided';
	}
	const answer = ServerStopDocumentSchema.safeParse(document);
	if (!answer.success) return 'undecided';
	switch (answer.data.result.outcome) {
		case 'busy':
		case 'failed':
		case 'stopped':
		case 'unmanaged':
			return answer.data.result.outcome;
		default:
			return 'undecided';
	}
}
