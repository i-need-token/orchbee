import { execFile } from 'node:child_process';

import { z } from 'zod';

/** A tier read answers from the Server within seconds; this bounds one that hangs. */
const STATUS_DEADLINE_MILLISECONDS = 15_000;
/** The restart's successor has its own budget, and the wait for its endpoint follows it. */
const RESTART_DEADLINE_MILLISECONDS = 120_000;

const RestartTierDocumentSchema = z.looseObject({
	result: z.looseObject({
		server: z.looseObject({
			restart: z
				.looseObject({
					hosts: z.array(z.looseObject({ tier: z.string() })).optional(),
					tier: z.string(),
				})
				.optional(),
		}),
	}),
	schema: z.literal('orchbee.status.server'),
});

const RestartDocumentSchema = z.looseObject({
	result: z.looseObject({ outcome: z.string() }),
	schema: z.literal('orchbee.server.restart'),
});

/**
 * What the active version's Client did for a switch to it: `not-needed` when it joins the Session
 * and its restart tier asks for nothing, `restarted` when it restarted the Session's Server and
 * Worker, keeping every terminal, `failed` when it could not, so the update waits, and
 * `host-required` when a Terminal Host must be replaced, which no restart keeping its terminals can
 * do: the person confirms a restart of every terminal first.
 */
export type UpdateRestart = 'failed' | 'host-required' | 'not-needed' | 'restarted';

/**
 * Decides and runs the Session restart a switch to the active version needs, through that version's
 * Client `executable`: the newer side reads the tier with `status server` and restarts with
 * `server restart`. Only Windows restarts a Session while terminals run; elsewhere a Session the
 * version cannot join waits for the automatic move.
 */
export async function restartForUpdate(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
	joins: boolean,
	platform: NodeJS.Platform,
): Promise<UpdateRestart> {
	if (platform !== 'win32') return joins ? 'not-needed' : 'failed';
	const restart = await readRestart(executable, sessionArguments, environment);
	if (restart?.hosts?.some((host) => host.tier === 'required') === true) return 'host-required';
	if (joins && restart?.tier !== 'recommended' && restart?.tier !== 'required') return 'not-needed';
	const outcome = await run(
		executable,
		[...sessionArguments, 'server', 'restart', '--format', 'json'],
		environment,
		RESTART_DEADLINE_MILLISECONDS,
	);
	const answer = RestartDocumentSchema.safeParse(outcome);
	return answer.success && answer.data.result.outcome === 'restarted' ? 'restarted' : 'failed';
}

async function readRestart(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<
	| { readonly hosts?: readonly { readonly tier: string }[] | undefined; readonly tier: string }
	| undefined
> {
	const document = await run(
		executable,
		[...sessionArguments, 'status', 'server', '--format', 'json'],
		environment,
		STATUS_DEADLINE_MILLISECONDS,
	);
	const answer = RestartTierDocumentSchema.safeParse(document);
	return answer.success ? answer.data.result.server.restart : undefined;
}

/** Runs one command and parses its standard output as JSON; anything unreadable is `undefined`. */
function run(
	executable: string,
	args: readonly string[],
	environment: NodeJS.ProcessEnv,
	timeout: number,
): Promise<unknown> {
	return new Promise((resolve) => {
		execFile(
			executable,
			args,
			{ env: environment, timeout, windowsHide: true },
			(_error, stdout) => {
				try {
					resolve(JSON.parse(stdout));
				} catch {
					resolve(undefined);
				}
			},
		);
	});
}
