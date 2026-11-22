import { execFile } from 'node:child_process';

import { z } from 'zod';

/** A read answers from the Server or the Session's record; this bounds one that hangs. */
const COMMAND_DEADLINE_MILLISECONDS = 10_000;

const ServerStatusDocumentSchema = z.looseObject({
	result: z.looseObject({
		server: z.looseObject({
			movedTo: z.string().optional(),
			state: z.string(),
		}),
	}),
	schema: z.literal('orchbee.status.server'),
});

/**
 * What `status server` says of the selected Session: `running` when a Server answers, `moved` when
 * the automatic update ended it to run a newer installed version, which the next start runs, and
 * `absent` otherwise, an answer that cannot be read included.
 */
export type ServerState = 'absent' | 'moved' | 'running';

/** Reads the selected Session's state with `status server`, which never starts a Session. */
export function readServerState(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<ServerState> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'status', 'server', '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			(_error, stdout) => resolve(interpretServerState(stdout)),
		);
	});
}

function interpretServerState(stdout: string): ServerState {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return 'absent';
	}
	const answer = ServerStatusDocumentSchema.safeParse(document);
	if (!answer.success) return 'absent';
	const { movedTo, state } = answer.data.result.server;
	if (state === 'running') return 'running';
	return movedTo === undefined ? 'absent' : 'moved';
}
