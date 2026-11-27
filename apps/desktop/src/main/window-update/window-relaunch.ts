/**
 * The window's relaunch through the window launcher: the switches it hands the launcher, the place
 * a relaunched window opens at, the launcher protocol variable an announced window reads, and when
 * such a window gives up so the launcher can start the version it replaced
 * (`apps/orchbee-desktop-launcher/README.md`).
 */
import path from 'node:path';

import {
	DesktopRelaunchPlaceSchema,
	type DesktopRelaunchPlace,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';

import { locateApplicationBundle } from './window-update-offer';

// Cross-build contract: every window launcher from the window self-update release on reads the
// first two and passes the place switches on unchanged; the macOS launcher also reads the third.
// Releases up to 0.1.0-beta.24 spell each with a `-v1` suffix. A window sends the current spelling,
// since the launcher it starts and the version it switches to are never older than itself, and
// reads the place in either spelling.
const RELAUNCH_AFTER_SWITCH = '--relaunch-after';
const REPLACE_APPLICATION_SWITCH = '--replace-application=';
const SELECT_TERMINAL_SWITCH = '--select-terminal=';
const SELECT_WORKSPACE_SWITCH = '--select-workspace=';
const EARLIER_SELECT_TERMINAL_SWITCH = '--select-terminal-v1=';
const EARLIER_SELECT_WORKSPACE_SWITCH = '--select-workspace-v1=';
/**
 * The launcher protocol variable, which carries the announcing launcher's process id, and its
 * earlier spelling, which launchers up to 0.1.0-beta.24 set instead.
 */
const RELAUNCH_ENVIRONMENT_KEYS = ['ORCHBEE_RELAUNCH', 'ORCHBEE_RELAUNCH_V1'] as const;
/** How long after its start an announced window still gives way to the version it replaced. */
const ANNOUNCED_REFUSAL_WINDOW_MILLISECONDS = 90_000;
const PROCESS_ID = /^[1-9]\d{0,9}$/u;

/** The value of the first argument that starts with either spelling of a switch. */
function switchValue(
	argumentsList: readonly string[],
	spellings: readonly [string, string],
): string | undefined {
	for (const argument of argumentsList) {
		const prefix = spellings.find((spelling) => argument.startsWith(spelling));
		if (prefix !== undefined) return argument.slice(prefix.length);
	}
	return undefined;
}

/**
 * The place a relaunch named on the command line: the terminal, else the Workspace, each only when
 * its identity parses; null without either.
 */
export function readRelaunchPlace(argumentsList: readonly string[]): DesktopRelaunchPlace | null {
	const terminal = DesktopRelaunchPlaceSchema.safeParse({
		kind: 'terminal',
		presentationId: switchValue(argumentsList, [
			SELECT_TERMINAL_SWITCH,
			EARLIER_SELECT_TERMINAL_SWITCH,
		]),
	});
	if (terminal.success) return terminal.data;
	const workspace = DesktopRelaunchPlaceSchema.safeParse({
		kind: 'workspace',
		workspaceId: switchValue(argumentsList, [
			SELECT_WORKSPACE_SWITCH,
			EARLIER_SELECT_WORKSPACE_SWITCH,
		]),
	});
	return workspace.success ? workspace.data : null;
}

/**
 * What the window launcher starts with: the launcher's two switches first, on macOS the application
 * it replaces next, then the place, if any.
 */
function windowRelaunchArguments(
	processId: number,
	application: string | null,
	place: DesktopRelaunchPlace | null,
): string[] {
	const relaunch = [RELAUNCH_AFTER_SWITCH, String(processId)];
	if (application !== null) relaunch.push(`${REPLACE_APPLICATION_SWITCH}${application}`);
	if (place === null) return relaunch;
	return place.kind === 'terminal'
		? [...relaunch, `${SELECT_TERMINAL_SWITCH}${place.presentationId}`]
		: [...relaunch, `${SELECT_WORKSPACE_SWITCH}${place.workspaceId}`];
}

/**
 * Removes the launcher protocol variable in both spellings from `environment`, so nothing this
 * window starts inherits it, and says whether either announced this window: its value is the
 * parent's process id, so a stale inherited variable announces nothing.
 */
export function takeRelaunchAnnouncement(
	environment: NodeJS.ProcessEnv,
	parentProcessId: number,
): boolean {
	let announced = false;
	for (const key of RELAUNCH_ENVIRONMENT_KEYS) {
		const value = environment[key];
		delete environment[key];
		if (value !== undefined && PROCESS_ID.test(value) && Number(value) === parentProcessId) {
			announced = true;
		}
	}
	return announced;
}

/**
 * Whether an announced window exits with status 1 instead of showing its first `server start`
 * refusal: only a Session too old for it, answered within 90 seconds of its start, so its launcher
 * starts the window version it replaced.
 */
export function givesWayToReplacedVersion(input: {
	readonly announced: boolean;
	readonly elapsedMilliseconds: number;
	readonly refusal: DesktopSessionRefusal | null;
}): boolean {
	return (
		input.announced &&
		input.elapsedMilliseconds <= ANNOUNCED_REFUSAL_WINDOW_MILLISECONDS &&
		input.refusal?.reason === 'session-too-old'
	);
}

export interface WindowRelaunchOptions {
	/** `desktop`, which holds the window entry on Windows and the window versions everywhere. */
	readonly desktopRoot: string;
	/** This window's program: the entry has its file name, and on macOS it names the bundle. */
	readonly executablePath: string;
	readonly place: DesktopRelaunchPlace | null;
	readonly platform: NodeJS.Platform;
	readonly processId: number;
	/** Quits as Quit Orchbee does, leaving the terminals running. */
	readonly quit: () => void;
	/** Starts `program` detached, so it outlives this process; resolves whether it started. */
	readonly start: (
		program: string,
		argumentsList: readonly string[],
		workingDirectory: string,
	) => Promise<boolean>;
	/**
	 * On macOS, where the application bundle is the entry, the window launcher in the cohort of the
	 * version this window switches to; null on Windows, which starts the entry beside the versions.
	 */
	readonly versionLauncher: string | null;
}

/**
 * Starts the window launcher, which waits for this window to end and then starts the active window
 * version at the same place, and quits once it has started. On Windows that is the window entry; on
 * macOS the launcher of the version this window switches to, which first replaces the application
 * bundle this window runs from. A window whose launcher cannot start keeps running and says so;
 * nothing else changes. Changing this (the relaunch and version switch) needs a manual real-window
 * self-test before merging: the relaunch must reach the new version. Its steps are in
 * `apps/desktop/README.md`, Window Update, Manual self-test.
 */
export async function relaunchWindow(options: WindowRelaunchOptions): Promise<boolean> {
	const paths = options.platform === 'win32' ? path.win32 : path.posix;
	let program: string;
	let application: string | null = null;
	if (options.platform === 'darwin') {
		application = locateApplicationBundle(options.executablePath);
		if (options.versionLauncher === null || application === null) return false;
		program = options.versionLauncher;
	} else {
		program = paths.join(options.desktopRoot, paths.basename(options.executablePath));
	}
	const argumentsList = windowRelaunchArguments(options.processId, application, options.place);
	const started = await options
		.start(program, argumentsList, options.desktopRoot)
		.catch(() => false);
	if (started) options.quit();
	return started;
}
