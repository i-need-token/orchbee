import { execFile } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';

import { z } from 'zod';

/** A tier read answers from the Server within seconds; this bounds one that hangs. */
const STATUS_DEADLINE_MILLISECONDS = 15_000;
/** The restart's successor has its own budget, and the wait for its endpoint follows it. */
const RESTART_DEADLINE_MILLISECONDS = 120_000;
/** How often the moves off a Host the version must replace are read: one terminal moves per turn. */
const SETTLE_PERIOD_MILLISECONDS = 2_000;
/** How long the count of terminals left stays unchanged before the rest count as unable to move. */
const SETTLE_STEADY_MILLISECONDS = 6_000;
/** How long the moves may take before the terminals left are asked about. */
const SETTLE_BUDGET_MILLISECONDS = 180_000;

const RestartTierSchema = z.looseObject({
	hosts: z
		.array(
			z.looseObject({
				moves: z.boolean().optional(),
				terminals: z.number().int().nonnegative().optional(),
				tier: z.string(),
			}),
		)
		.optional(),
	tier: z.string(),
});

type RestartTier = z.infer<typeof RestartTierSchema>;

const RestartTierDocumentSchema = z.looseObject({
	result: z.looseObject({
		server: z.looseObject({ restart: RestartTierSchema.optional() }),
	}),
	schema: z.literal('orchbee.status.server'),
});

const RestartDocumentSchema = z.looseObject({
	result: z.looseObject({ outcome: z.string() }),
	schema: z.literal('orchbee.server.restart'),
});

/**
 * Terminals that could not move off a Terminal Host the active version must replace, once the
 * Session runs that version: the person confirms a restart of every terminal, or keeps them there.
 */
export interface TerminalsStaying {
	readonly kind: 'terminals-staying';
	readonly terminals: number;
}

/**
 * What the active version's Client did for a switch to it: `not-needed` when it joins the Session
 * and its restart tier asks for nothing, `restarted` when it restarted the Session's Server and
 * Worker, keeping every terminal, `failed` when it could not, so the update waits, `host-required`
 * when a Terminal Host must be replaced whose terminals that version's Worker does not move, which
 * no restart keeping its terminals can do: the person confirms a restart of every terminal first,
 * and {@link TerminalsStaying} once the moves off such a Host settled with terminals left on it.
 */
export type UpdateRestart =
	| 'failed'
	| 'host-required'
	| 'not-needed'
	| 'restarted'
	| TerminalsStaying;

/**
 * Decides and runs the Session restart a switch to the active version needs, through that version's
 * Client `executable`: the newer side reads the tier with `status server` and restarts with
 * `server restart`. Once the Session runs that version, a `required` Host whose terminals move is
 * read every two seconds until those Hosts run no terminal or their count held for six seconds. Only
 * Windows and macOS restart a Session while terminals run; elsewhere a Session the version cannot
 * join waits for the automatic move.
 */
export async function restartForUpdate(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
	joins: boolean,
	platform: NodeJS.Platform,
): Promise<UpdateRestart> {
	if (platform !== 'win32' && platform !== 'darwin') return joins ? 'not-needed' : 'failed';
	const read = (): Promise<RestartTier | undefined> =>
		readRestart(executable, sessionArguments, environment);
	const restart = await read();
	const hosts = restart?.hosts ?? [];
	if (hosts.some((host) => host.tier === 'required' && host.moves !== true)) return 'host-required';
	const moving = restart !== undefined && staying(restart) > 0;
	if (joins && restart?.tier !== 'recommended' && restart?.tier !== 'required') {
		return moving ? left(await settle(read, performance.now()), 'not-needed') : 'not-needed';
	}
	const outcome = await run(
		executable,
		[...sessionArguments, 'server', 'restart', '--format', 'json'],
		environment,
		RESTART_DEADLINE_MILLISECONDS,
	);
	const answer = RestartDocumentSchema.safeParse(outcome);
	if (!answer.success || answer.data.result.outcome !== 'restarted') return 'failed';
	return moving ? left(await settle(read, performance.now()), 'restarted') : 'restarted';
}

function left(terminals: number, none: 'not-needed' | 'restarted'): UpdateRestart {
	return terminals === 0 ? none : { kind: 'terminals-staying', terminals };
}

/**
 * The terminals left on the Hosts the active version must replace once its Worker's moves settle:
 * none, or a count that held for {@link SETTLE_STEADY_MILLISECONDS}, or the last one read when
 * {@link SETTLE_BUDGET_MILLISECONDS} since `began` passed. A read that fails changes nothing.
 */
async function settle(
	read: () => Promise<RestartTier | undefined>,
	began: number,
	last?: { readonly count: number; readonly since: number },
): Promise<number> {
	const restart = await read();
	const now = performance.now();
	let held = last;
	if (restart !== undefined) {
		const count = staying(restart);
		if (count === 0) return 0;
		if (held?.count !== count) held = { count, since: now };
		else if (now - held.since >= SETTLE_STEADY_MILLISECONDS) return count;
	}
	if (now - began >= SETTLE_BUDGET_MILLISECONDS) return held?.count ?? 0;
	await delay(SETTLE_PERIOD_MILLISECONDS);
	return settle(read, began, held);
}

/** The terminals the Hosts `restart` names `required` run. */
function staying(restart: RestartTier): number {
	return (restart.hosts ?? [])
		.filter((host) => host.tier === 'required')
		.reduce((sum, host) => sum + (host.terminals ?? 0), 0);
}

async function readRestart(
	executable: string,
	sessionArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
): Promise<RestartTier | undefined> {
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
