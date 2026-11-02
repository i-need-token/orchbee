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

const COHORT_DIRECTORY_PATTERN = /^cohort-[0-9a-f]{16}$/u;
/** The CLI's Session name grammar, which also keeps a name inside the store as a file name. */
const SESSION_NAME_PATTERN = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,64}$/u;
const SESSION_MARKER_SUFFIX = '.session.json';
const SessionMarkerSchema = z.strictObject({
	cohort: z.string().regex(COHORT_DIRECTORY_PATTERN),
});
/** `server stop` waits up to 65 seconds by default. */
const SESSION_STOP_TIMEOUT_MILLISECONDS = 70_000;

/**
 * Builds the checkout's native cohort and stages it once per distinct build under
 * `target/desktop-development`, returning the staged `orchbee`. A Session refuses a Client of
 * another cohort, so a development Session that an earlier cohort started is stopped by that cohort
 * first; cohorts no development Session names any more are removed.
 */
export async function prepareDesktopDevelopmentCohort(input: {
	readonly environment: NodeJS.ProcessEnv;
	readonly logger: DesktopDevelopmentRuntimeLogger;
	readonly sourceRoot: string;
}): Promise<string> {
	const sourceRoot = await realpath(input.sourceRoot);
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
	);
	await run('zig', ['build', '-Doptimize=ReleaseSafe'], hostRoot);
	const key = createHash('sha256');
	for (const file of [
		path.join(sourceRoot, 'target', 'release', `orchbee${suffix}`),
		path.join(sourceRoot, 'target', 'release', `orchbee-server${suffix}`),
		path.join(sourceRoot, 'target', 'release', `orchbee-worker${suffix}`),
		path.join(hostRoot, 'zig-out', 'bin', `orchbee-terminal-host${suffix}`),
		path.join(hostRoot, 'windows-conpty.json'),
		path.join(
			sourceRoot,
			'apps',
			'desktop',
			'resources',
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
			[path.join(sourceRoot, 'scripts', 'stage-local-runtime.mjs'), '--output', cohort],
			sourceRoot,
		);
	}
	const executable = path.join(cohort, `orchbee${suffix}`);
	const session =
		input.environment['ORCHBEE_DESKTOP_SESSION'] ??
		resolveDesktopSessionName(
			resolveDesktopDistributionProfile(
				'dev',
				createDesktopDevelopmentScope(sourceRoot, process.platform),
			),
		);
	if (session === undefined || !SESSION_NAME_PATTERN.test(session)) return executable;
	const markerPath = path.join(store, `${session}${SESSION_MARKER_SUFFIX}`);
	const previous = await readSessionMarker(markerPath);
	if (previous !== undefined && previous !== cohortName) {
		const previousExecutable = path.join(store, previous, `orchbee${suffix}`);
		if (await exists(previousExecutable)) {
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
	await writeFile(markerPath, `${JSON.stringify({ cohort: cohortName })}\n`, 'utf8');
	await removeUnnamedCohorts(store, cohortName, input.logger);
	return executable;
}

async function readSessionMarker(markerPath: string): Promise<string | undefined> {
	let text: string;
	try {
		text = await readFile(markerPath, 'utf8');
	} catch {
		return undefined;
	}
	try {
		const parsed = SessionMarkerSchema.safeParse(JSON.parse(text));
		return parsed.success ? parsed.data.cohort : undefined;
	} catch {
		return undefined;
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
		const cohort = await readSessionMarker(path.join(store, entry));
		if (cohort !== undefined) named.add(cohort);
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

async function exists(filePath: string): Promise<boolean> {
	try {
		await access(filePath);
		return true;
	} catch {
		return false;
	}
}

function run(command: string, arguments_: readonly string[], cwd: string): Promise<void> {
	return new Promise((resolve, reject) => {
		const child = spawn(command, [...arguments_], { cwd, stdio: 'inherit', windowsHide: true });
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
