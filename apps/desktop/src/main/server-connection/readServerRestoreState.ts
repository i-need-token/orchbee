import { execFile } from 'node:child_process';

import { z } from 'zod';

/** The command's own budget is 20 seconds; this only bounds a process that stopped answering. */
const COMMAND_DEADLINE_MILLISECONDS = 25_000;

const ServerStartDocumentSchema = z.looseObject({
	result: z.looseObject({ restoreNotice: z.enum(['recovered', 'empty']).nullable() }),
});

/**
 * Activates the Session and says whether its restore lost everything. Only the activating process
 * learns that, and a relay's bytes stay opaque to Main, so this runs before the relay does. A
 * Session that cannot answer is reported the same way, because neither may be written to unasked.
 */
export function readServerRestoreState(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<'intact' | 'lost-or-unknown'> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'server', 'start', '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			(error, stdout) => {
				if (error !== null) {
					resolve('lost-or-unknown');
					return;
				}
				try {
					const document = ServerStartDocumentSchema.parse(JSON.parse(stdout));
					resolve(document.result.restoreNotice === 'empty' ? 'lost-or-unknown' : 'intact');
				} catch {
					resolve('lost-or-unknown');
				}
			},
		);
	});
}
