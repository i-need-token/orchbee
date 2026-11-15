/**
 * The window's relaunch through the window entry: the switches it hands the window launcher, the
 * place a relaunched window opens at, the launcher protocol variable an announced window reads, and
 * when such a window gives up so the launcher can start the version it replaced
 * (`apps/orchbee-desktop-launcher/README.md`).
 */
import path from 'node:path';

import {
	DesktopRelaunchPlaceSchema,
	type DesktopRelaunchPlace,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';

// Cross-build contract: every window launcher from the window self-update release on reads the
// first two and passes the place switches on unchanged.
const RELAUNCH_AFTER_SWITCH = '--relaunch-after-v1';
const SELECT_TERMINAL_SWITCH = '--select-terminal-v1=';
const SELECT_WORKSPACE_SWITCH = '--select-workspace-v1=';
/** The launcher protocol variable, which carries the announcing launcher's process id. */
const RELAUNCH_ENVIRONMENT_KEY = 'ORCHBEE_RELAUNCH_V1';
/** How long after its start an announced window still gives way to the version it replaced. */
const ANNOUNCED_REFUSAL_WINDOW_MILLISECONDS = 90_000;
const PROCESS_ID = /^[1-9]\d{0,9}$/u;

function switchValue(argumentsList: readonly string[], prefix: string): string | undefined {
	return argumentsList.find((argument) => argument.startsWith(prefix))?.slice(prefix.length);
}

/**
 * The place a relaunch named on the command line: the terminal, else the Workspace, each only when
 * its identity parses; null without either.
 */
export function readRelaunchPlace(argumentsList: readonly string[]): DesktopRelaunchPlace | null {
	const terminal = DesktopRelaunchPlaceSchema.safeParse({
		kind: 'terminal',
		presentationId: switchValue(argumentsList, SELECT_TERMINAL_SWITCH),
	});
	if (terminal.success) return terminal.data;
	const workspace = DesktopRelaunchPlaceSchema.safeParse({
		kind: 'workspace',
		workspaceId: switchValue(argumentsList, SELECT_WORKSPACE_SWITCH),
	});
	return workspace.success ? workspace.data : null;
}

/** What the window entry starts with: the launcher's two switches first, then the place, if any. */
function windowRelaunchArguments(processId: number, place: DesktopRelaunchPlace | null): string[] {
	const relaunch = [RELAUNCH_AFTER_SWITCH, String(processId)];
	if (place === null) return relaunch;
	return place.kind === 'terminal'
		? [...relaunch, `${SELECT_TERMINAL_SWITCH}${place.presentationId}`]
		: [...relaunch, `${SELECT_WORKSPACE_SWITCH}${place.workspaceId}`];
}

/**
 * Removes the launcher protocol variable from `environment`, so nothing this window starts inherits
 * it, and says whether it announced this window: its value is the parent's process id, so a stale
 * inherited variable announces nothing.
 */
export function takeRelaunchAnnouncement(
	environment: NodeJS.ProcessEnv,
	parentProcessId: number,
): boolean {
	const value = environment[RELAUNCH_ENVIRONMENT_KEY];
	delete environment[RELAUNCH_ENVIRONMENT_KEY];
	return value !== undefined && PROCESS_ID.test(value) && Number(value) === parentProcessId;
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
	/** `desktop\`, which holds the window entry. */
	readonly desktopRoot: string;
	/** This window's program, whose file name the entry has. */
	readonly executablePath: string;
	readonly place: DesktopRelaunchPlace | null;
	readonly platform: NodeJS.Platform;
	readonly processId: number;
	/** Quits as Quit Orchbee, Keep Terminals Running does. */
	readonly quit: () => void;
	/** Starts `program` detached, so it outlives this process; resolves whether it started. */
	readonly start: (
		program: string,
		argumentsList: readonly string[],
		workingDirectory: string,
	) => Promise<boolean>;
}

/**
 * Starts the window entry, which waits for this window to end and then starts the active window
 * version at the same place, and quits once it has started. A window whose entry cannot start keeps
 * running and says so; nothing else changes.
 */
export async function relaunchWindow(options: WindowRelaunchOptions): Promise<boolean> {
	const paths = options.platform === 'win32' ? path.win32 : path.posix;
	const entry = paths.join(options.desktopRoot, paths.basename(options.executablePath));
	const started = await options
		.start(entry, windowRelaunchArguments(options.processId, options.place), options.desktopRoot)
		.catch(() => false);
	if (started) options.quit();
	return started;
}
