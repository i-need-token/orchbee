import { execFile } from 'node:child_process';

import { z } from 'zod';

/** A read answers from the Server within seconds; this bounds one that hangs. */
const COMMAND_DEADLINE_MILLISECONDS = 15_000;

const SessionMovesDocumentSchema = z.looseObject({
	result: z.looseObject({
		server: z.looseObject({
			moves: z
				.looseObject({
					stopped: z.looseObject({}).optional(),
					version: z.string().regex(/^[!-~]{1,64}$/u),
				})
				.optional(),
			state: z.string(),
		}),
	}),
	schema: z.literal('orchbee.status.server'),
});

/**
 * What a running Session's `status server` says about the terminals that Terminal Hosts older than
 * its Worker still run: null while none does, else the oldest such Host's `version` and whether a
 * failed move `stopped` moving them, which only a restart's Worker moves again.
 */
export type SessionMoves = { readonly stopped: boolean; readonly version: string } | null;

/**
 * Reads the selected Session's moves through the Client `executable` with `status server`, which
 * never starts a Session; `undefined` when the answer cannot be read or the Session does not run.
 */
export function readSessionMoves(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<SessionMoves | undefined> {
	return new Promise((resolve) => {
		execFile(
			executable,
			[...sessionArguments, 'status', 'server', '--format', 'json'],
			{ env: environment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
			(_error, stdout) => resolve(interpretSessionMoves(stdout)),
		);
	});
}

function interpretSessionMoves(stdout: string): SessionMoves | undefined {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return undefined;
	}
	const answer = SessionMovesDocumentSchema.safeParse(document);
	if (!answer.success || answer.data.result.server.state !== 'running') return undefined;
	const moves = answer.data.result.server.moves;
	return moves === undefined
		? null
		: { stopped: moves.stopped !== undefined, version: moves.version };
}
