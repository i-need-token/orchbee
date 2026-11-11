import { execFile, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { access, mkdir, readFile, readdir, realpath, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
	resolveDesktopSessionName,
} from '../distribution';
import type { DesktopDevelopmentRuntimeLogger } from './DesktopDevelopmentRuntime';
import {
	type DevelopmentSessionMarker,
	DevelopmentSessionMarkerSchema,
	type DevelopmentSessionObservation,
	keepDevelopmentSession,
	keepsDevelopmentSession,
} from './development-session-reuse';

const COHORT_DIRECTORY_PATTERN = /^cohort-[0-9a-f]{16}$/u;
/**
 * The CLI's Session name grammar, which also keeps a name inside the store as a file name: 1–64
 * ASCII letters, digits, `.`, `_` and `-`, neither starting nor ending with `.`, and no Windows
 * device name with or without an extension. Letter case never distinguishes two Sessions.
 */
// Cross-language contract: search for `session-name-grammar` before changing this rule, then
// update every occurrence together.
const SESSION_NAME_PATTERN =
	/^(?!\.)(?!.*\.$)(?!(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$))[A-Za-z0-9._-]{1,64}$/iu;
const SESSION_MARKER_SUFFIX = '.session.json';
/** `running` is null while the Session starts, deletes, or cannot be read. */
const SessionListSchema = z.object({
	result: z.object({
		sessions: z.array(
			z.object({
				name: z.string(),
				pid: z.int().positive().nullable(),
				running: z.boolean().nullable(),
			}),
		),
	}),
	schema: z.literal('orchbee.session.list'),
});
/** `server stop` waits up to 65 seconds by default. */
const SESSION_STOP_TIMEOUT_MILLISECONDS = 70_000;
/** `session list` reads under its own five-second deadline. */
const SESSION_LIST_TIMEOUT_MILLISECONDS = 10_000;

/**
 * Builds the checkout's native cohort and stages it once per distinct build under
 * `target/desktop-development`, returning the staged Client. A development Session that an earlier
 * cohort started keeps running when that cohort's Server, Worker, and Terminal Host are unchanged,
 * since the new Client joins it by protocol fingerprint; otherwise that cohort stops it first.
 * Cohorts no development Session marker names any more are removed.
 */
export async function prepareDesktopDevelopmentCohort(input: {
	readonly environment: NodeJS.ProcessEnv;
	readonly logger: DesktopDevelopmentRuntimeLogger;
	readonly sourceRoot: string;
}): Promise<string> {
	const sourceRoot = await realpath(input.sourceRoot);
	const profile = resolveDesktopDistributionProfile(
		'dev',
		createDesktopDevelopmentScope(sourceRoot, process.platform),
	);
	// Every executable compiles in the environment stem, which the handshake compares, so a release
	// Client in one of this Desktop's terminals is refused on every platform. The Client, Server and
	// Worker find their siblings by the same stem in their file names.
	const environmentStem = profile.artifactStem;
	const suffix = process.platform === 'win32' ? '.exe' : '';
	const hostRoot = path.join(sourceRoot, 'apps', 'terminal-host');
	await run(
		'cargo',
		[
			'build',
			'--release',
			'--locked',
			'-p',
			'orchbee',
			'-p',
			'orchbee-server',
			'-p',
			'orchbee-worker',
		],
		sourceRoot,
		{ ...process.env, ORCHBEE_ARTIFACT_STEM: environmentStem },
	);
	// The Host carries the cohort's environment, so it builds into its own prefix and leaves the
	// test Host in `zig-out` alone.
	const hostPrefix = path.join(sourceRoot, 'target', 'desktop-development', 'host');
	await run(
		'zig',
		['build', '-Doptimize=ReleaseSafe', `-Denvironment=${environmentStem}`, '--prefix', hostPrefix],
		hostRoot,
	);
	const key = createHash('sha256').update(environmentStem).update('\0');
	for (const file of [
		path.join(sourceRoot, 'target', 'release', `orchbee${suffix}`),
		path.join(sourceRoot, 'target', 'release', `orchbee-server${suffix}`),
		path.join(sourceRoot, 'target', 'release', `orchbee-worker${suffix}`),
		path.join(hostPrefix, 'bin', `orchbee-terminal-host${suffix}`),
		path.join(hostRoot, 'windows-conpty.json'),
		path.join(
			sourceRoot,
			'crates',
			'agent-integration-writer',
			'agent-observation-adapters',
			'capsule.json',
		),
	]) {
		key.update(await readFile(file)).update('\0');
	}
	const store = path.join(sourceRoot, 'target', 'desktop-development');
	const cohortName = `cohort-${key.digest('hex').slice(0, 16)}`;
	const cohort = path.join(store, cohortName);
	await mkdir(store, { recursive: true });
	if (await exists(path.join(cohort, 'cohort.json'))) {
		input.logger.info(`Reusing the staged native cohort ${cohortName}.`);
	} else {
		// Staging publishes `cohort.json` last, so a directory without it is an interrupted attempt.
		await rm(cohort, { force: true, recursive: true });
		await run(
			process.execPath,
			[
				path.join(sourceRoot, 'scripts', 'stage-local-runtime.mjs'),
				'--output',
				cohort,
				'--artifact-stem',
				environmentStem,
			],
			sourceRoot,
		);
	}
	const executable = path.join(cohort, `${environmentStem}${suffix}`);
	const session =
		input.environment['ORCHBEE_DESKTOP_SESSION'] ?? resolveDesktopSessionName(profile);
	if (session === undefined || !SESSION_NAME_PATTERN.test(session)) return executable;
	const markerPath = path.join(store, `${session.toLowerCase()}${SESSION_MARKER_SUFFIX}`);
	const marker = await readSessionMarker(markerPath);
	let next: DevelopmentSessionMarker = { client: cohortName, cohort: cohortName };
	let removesUnnamedCohorts = true;
	if (marker !== undefined && (marker.cohort !== cohortName || marker.client !== cohortName)) {
		const previous = marker.cohort;
		if (
			keepsDevelopmentSession(
				await readCohortManifest(path.join(store, previous)),
				await readCohortManifest(cohort),
			)
		) {
			const observation = await observeSession(executable, session, input.environment);
			next = keepDevelopmentSession(marker, cohortName, observation);
			// While nothing says which cohort runs the Session, none is removed.
			removesUnnamedCohorts = observation.kind !== 'unknown';
			if (observation.kind !== 'stopped') {
				input.logger.info(
					`Keeping Session ${session}: its Server, Worker and Terminal Host are unchanged.`,
				);
			}
		} else {
			const previousExecutable = await findCohortClient(
				path.join(store, previous),
				environmentStem,
				suffix,
			);
			if (previousExecutable !== undefined) {
				input.logger.info(`Stopping Session ${session}, which ${previous} started…`);
				const failure = await stopSession(previousExecutable, session, input.environment);
				if (failure !== undefined) {
					input.logger.warn(
						`Session ${session} still runs ${previous} and could not be stopped (${failure}), ` +
							'so this window cannot join it. Stop it with ' +
							`"${previousExecutable}" --session ${session} server stop --confirm, then restart.`,
					);
					return executable;
				}
			}
		}
	}
	await writeFile(markerPath, `${JSON.stringify(next)}\n`, 'utf8');
	if (removesUnnamedCohorts) await removeUnnamedCohorts(store, cohortName, input.logger);
	return executable;
}

async function readSessionMarker(
	markerPath: string,
): Promise<DevelopmentSessionMarker | undefined> {
	let text: string;
	try {
		text = await readFile(markerPath, 'utf8');
	} catch {
		return undefined;
	}
	try {
		const parsed = DevelopmentSessionMarkerSchema.safeParse(JSON.parse(text));
		return parsed.success ? parsed.data : undefined;
	} catch {
		return undefined;
	}
}

/** The staged `cohort.json`, or nothing when it cannot be read. */
async function readCohortManifest(cohort: string): Promise<unknown> {
	try {
		const document: unknown = JSON.parse(await readFile(path.join(cohort, 'cohort.json'), 'utf8'));
		return document;
	} catch {
		return undefined;
	}
}

/** What `session list` says about the named Session; one it leaves out has never started. */
function observeSession(
	executable: string,
	session: string,
	environment: NodeJS.ProcessEnv,
): Promise<DevelopmentSessionObservation> {
	return new Promise((resolve) => {
		execFile(
			executable,
			['session', 'list', '--format', 'json'],
			{ env: environment, timeout: SESSION_LIST_TIMEOUT_MILLISECONDS, windowsHide: true },
			(error, stdout) => {
				resolve(error === null ? readSessionObservation(stdout, session) : { kind: 'unknown' });
			},
		);
	});
}

function readSessionObservation(stdout: string, session: string): DevelopmentSessionObservation {
	try {
		const parsed = SessionListSchema.safeParse(JSON.parse(stdout));
		if (!parsed.success) return { kind: 'unknown' };
		const entry = parsed.data.result.sessions.find(
			(candidate) => candidate.name.toLowerCase() === session.toLowerCase(),
		);
		if (entry === undefined || entry.running === false) return { kind: 'stopped' };
		return entry.running === true && entry.pid !== null
			? { kind: 'running', pid: entry.pid }
			: { kind: 'unknown' };
	} catch {
		return { kind: 'unknown' };
	}
}

/** Returns why the stop failed, or nothing once the Session is stopped or was not running. */
function stopSession(
	executable: string,
	session: string,
	environment: NodeJS.ProcessEnv,
): Promise<string | undefined> {
	return new Promise((resolve) => {
		execFile(
			executable,
			['--session', session, 'server', 'stop', '--confirm', '--format', 'json'],
			{ env: environment, timeout: SESSION_STOP_TIMEOUT_MILLISECONDS, windowsHide: true },
			(error, stdout) => {
				if (error === null) {
					resolve(undefined);
					return;
				}
				const outcome = readStopOutcome(stdout);
				resolve(outcome === 'unavailable/runtime' ? undefined : (outcome ?? 'no stop result'));
			},
		);
	});
}

function readStopOutcome(stdout: string): string | undefined {
	try {
		const document: unknown = JSON.parse(stdout);
		const parsed = z
			.object({
				result: z.object({ outcome: z.string(), reason: z.string().nullish() }),
				schema: z.literal('orchbee.server.stop'),
			})
			.safeParse(document);
		if (!parsed.success) return undefined;
		const { outcome, reason } = parsed.data.result;
		return reason === undefined || reason === null ? outcome : `${outcome}/${reason}`;
	} catch {
		return undefined;
	}
}

async function removeUnnamedCohorts(
	store: string,
	current: string,
	logger: DesktopDevelopmentRuntimeLogger,
): Promise<void> {
	const entries = await readdir(store);
	const named = new Set([current]);
	for (const entry of entries) {
		if (!entry.endsWith(SESSION_MARKER_SUFFIX)) continue;
		const marker = await readSessionMarker(path.join(store, entry));
		if (marker !== undefined) named.add(marker.cohort).add(marker.client ?? marker.cohort);
	}
	for (const entry of entries) {
		if (!COHORT_DIRECTORY_PATTERN.test(entry) || named.has(entry)) continue;
		await rm(path.join(store, entry), { force: true, recursive: true }).catch((error: unknown) => {
			logger.warn(
				`Could not remove the unused native cohort ${entry}: ${error instanceof Error ? error.message : 'unknown failure'}`,
			);
		});
	}
}

/**
 * The Client a staged cohort holds. A cohort staged before ORC-382 holds `orchbee`, and the Session it
 * started still has to be stopped by that Client.
 */
async function findCohortClient(
	cohort: string,
	stem: string,
	suffix: string,
): Promise<string | undefined> {
	for (const name of [`${stem}${suffix}`, `orchbee${suffix}`]) {
		const candidate = path.join(cohort, name);
		if (await exists(candidate)) return candidate;
	}
	return undefined;
}

async function exists(filePath: string): Promise<boolean> {
	try {
		await access(filePath);
		return true;
	} catch {
		return false;
	}
}

function run(
	command: string,
	arguments_: readonly string[],
	cwd: string,
	environment?: NodeJS.ProcessEnv,
): Promise<void> {
	return new Promise((resolve, reject) => {
		const child = spawn(command, [...arguments_], {
			cwd,
			env: environment,
			stdio: 'inherit',
			windowsHide: true,
		});
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0) resolve();
			else
				reject(
					new Error(`${command} exited with code ${String(code)} and signal ${String(signal)}.`),
				);
		});
	});
}
