import { execFile } from 'node:child_process';

import { z } from 'zod';

/** The command's own budget is 20 seconds; this only bounds a process that stopped answering. */
const COMMAND_DEADLINE_MILLISECONDS = 25_000;

const ServerStartDocumentSchema = z.looseObject({
	result: z.looseObject({ restoreNotice: z.enum(['recovered', 'empty']).nullable() }),
});

/** The refusal `server start` answers until a Session it cannot prove stopped is recovered. */
const RecoveryRequiredFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({ reason: z.literal('recovery-required') }),
	}),
});

export type ServerRestoreState = 'intact' | 'lost-or-unknown' | 'recovery-required';

/**
 * Activates the Session and says whether its restore lost everything. Only the activating process
 * learns that, and a relay's bytes stay opaque to Main, so this runs before the relay does. A
 * Session that cannot answer is reported the same way, because neither may be written to unasked.
 * A Session refused until it is recovered is named, because only that answer says what would help.
 */
export function readServerRestoreState(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<ServerRestoreState> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'server', 'start', '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			(error, stdout) => {
				try {
					const document: unknown = JSON.parse(stdout);
					if (error !== null) {
						resolve(
							RecoveryRequiredFailureSchema.safeParse(document).success
								? 'recovery-required'
								: 'lost-or-unknown',
						);
						return;
					}
					const answer = ServerStartDocumentSchema.parse(document);
					resolve(answer.result.restoreNotice === 'empty' ? 'lost-or-unknown' : 'intact');
				} catch {
					resolve('lost-or-unknown');
				}
			},
		);
	});
}
