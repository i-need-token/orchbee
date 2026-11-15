import { execFile } from 'node:child_process';

import { interpretServerStartAnswer, type ServerStartAnswer } from './interpretServerStartAnswer';

/** The command's own budget is 20 seconds; this only bounds a process that stopped answering. */
const COMMAND_DEADLINE_MILLISECONDS = 25_000;

/**
 * Activates the Session through `executable` and says whether its restore lost everything and which
 * capabilities its handshake negotiated. Only the activating process learns that, and a relay's
 * bytes stay opaque to Main, so this runs before the relay does. A Session that cannot answer is
 * reported as a lost restore, because neither may be written to unasked. A refused start is named
 * with its reason, because only that answer says what would help.
 */
export function readServerStartAnswer(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<ServerStartAnswer> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'server', 'start', '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			(error, stdout) => {
				resolve(interpretServerStartAnswer(stdout, error !== null));
			},
		);
	});
}
