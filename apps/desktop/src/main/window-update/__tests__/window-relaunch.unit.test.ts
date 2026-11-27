import type { DesktopSessionRefusal } from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	givesWayToReplacedVersion,
	readRelaunchPlace,
	relaunchWindow,
	takeRelaunchAnnouncement,
} from '../window-relaunch';

const PRESENTATION = 'presentation-1';
const WORKSPACE = '0123456789abcdef0123456789abcdef';
const SESSION_TOO_OLD: DesktopSessionRefusal = {
	desktopVersion: '0.3.0',
	reason: 'session-too-old',
	sessionVersion: '0.1.0',
};

describe('window relaunch arguments', () => {
	it('names the terminal first, else the Workspace, and nothing without a valid place', () => {
		expect(
			readRelaunchPlace(['--select-workspace=' + WORKSPACE, '--select-terminal=' + PRESENTATION]),
		).toEqual({ kind: 'terminal', presentationId: PRESENTATION });
		expect(readRelaunchPlace(['--select-workspace=' + WORKSPACE])).toEqual({
			kind: 'workspace',
			workspaceId: WORKSPACE,
		});
		// A window up to 0.1.0-beta.24 names the place in the earlier spelling.
		expect(readRelaunchPlace(['--select-terminal-v1=' + PRESENTATION])).toEqual({
			kind: 'terminal',
			presentationId: PRESENTATION,
		});
		expect(readRelaunchPlace(['--select-workspace-v1=' + WORKSPACE])).toEqual({
			kind: 'workspace',
			workspaceId: WORKSPACE,
		});
		// A terminal identity that does not parse leaves the Workspace.
		expect(readRelaunchPlace(['--select-terminal=', '--select-workspace=' + WORKSPACE])).toEqual({
			kind: 'workspace',
			workspaceId: WORKSPACE,
		});
		for (const argumentsList of [
			[],
			['--select-workspace=00000000000000000000000000000000'],
			['--select-workspace=NOT-HEX'],
			['--select-terminal', PRESENTATION],
			['--select-terminal=-leading-dash'],
		]) {
			expect(readRelaunchPlace(argumentsList)).toBeNull();
		}
	});

	it.each([
		['a terminal', readRelaunchPlace(['--select-terminal=' + PRESENTATION])],
		['a Workspace', readRelaunchPlace(['--select-workspace=' + WORKSPACE])],
		['no place', null],
	])(
		'puts the launcher switch and the process id first, and %s reads back',
		async (_case, place) => {
			let argumentsList: readonly string[] = [];
			await relaunchWindow({
				desktopRoot: 'C:\\Orchbee\\desktop',
				executablePath: 'C:\\Orchbee\\desktop\\versions\\0.2.0\\orchbee-desktop.exe',
				place,
				platform: 'win32',
				processId: 4321,
				quit: () => undefined,
				start: (_program, started) => {
					argumentsList = started;
					return Promise.resolve(true);
				},
				versionLauncher: null,
			});
			expect(argumentsList.slice(0, 2)).toEqual(['--relaunch-after', '4321']);
			// Every place switch is one `=` argument, which the launcher passes on unchanged.
			expect(argumentsList.slice(2).every((argument) => argument.includes('='))).toBe(true);
			expect(readRelaunchPlace(argumentsList.slice(2))).toEqual(place);
		},
	);
});

describe('relaunch announcement', () => {
	it('always removes the variable, and announces only when it names the parent', () => {
		// Launchers up to 0.1.0-beta.24 set only the earlier spelling.
		for (const key of ['ORCHBEE_RELAUNCH', 'ORCHBEE_RELAUNCH_V1']) {
			for (const [value, announced] of [
				['1200', true],
				['1201', false],
				['01200', false],
				['+1200', false],
				['', false],
				[undefined, false],
			] as const) {
				const environment: NodeJS.ProcessEnv = { KEEP: 'yes' };
				if (value !== undefined) environment[key] = value;
				expect(takeRelaunchAnnouncement(environment, 1200)).toBe(announced);
				expect(environment).toEqual({ KEEP: 'yes' });
			}
		}
	});

	it('gives way only to a too-old Session answered within 90 seconds of an announced start', () => {
		expect(
			givesWayToReplacedVersion({
				announced: true,
				elapsedMilliseconds: 90_000,
				refusal: SESSION_TOO_OLD,
			}),
		).toBe(true);
		expect(
			givesWayToReplacedVersion({
				announced: true,
				elapsedMilliseconds: 90_001,
				refusal: SESSION_TOO_OLD,
			}),
		).toBe(false);
		expect(
			givesWayToReplacedVersion({
				announced: false,
				elapsedMilliseconds: 1_000,
				refusal: SESSION_TOO_OLD,
			}),
		).toBe(false);
		for (const refusal of [
			null,
			{ reason: 'recovery-required' },
			{ desktopVersion: '0.3.0', reason: 'desktop-too-old', sessionVersion: '0.4.0' },
		] as const) {
			expect(
				givesWayToReplacedVersion({ announced: true, elapsedMilliseconds: 1_000, refusal }),
			).toBe(false);
		}
	});
});

describe('relaunchWindow', () => {
	const place = readRelaunchPlace(['--select-terminal=' + PRESENTATION]);

	it('starts the entry beside the version folders from desktop and quits once it started', async () => {
		const quit = vi.fn<() => void>();
		const start = vi.fn<
			(
				program: string,
				argumentsList: readonly string[],
				workingDirectory: string,
			) => Promise<boolean>
		>(() => Promise.resolve(true));
		const started = await relaunchWindow({
			desktopRoot: 'C:\\Orchbee\\desktop',
			executablePath: 'C:\\Orchbee\\desktop\\versions\\0.2.0\\ob-test-desktop.exe',
			place,
			platform: 'win32',
			processId: 77,
			quit,
			start,
			versionLauncher: null,
		});
		expect(started).toBe(true);
		expect(start).toHaveBeenCalledWith(
			'C:\\Orchbee\\desktop\\ob-test-desktop.exe',
			['--relaunch-after', '77', '--select-terminal=' + PRESENTATION],
			'C:\\Orchbee\\desktop',
		);
		expect(quit).toHaveBeenCalledOnce();
	});

	// The macOS launcher of another build reads the third argument as the bundle it replaces.
	it('on macOS starts the version’s launcher and names the bundle it replaces third', async () => {
		const launcher =
			'/Users/a/.local/share/orchbee/desktop/versions/0.3.0/Orchbee.app/Contents/Resources/cohort/orchbee-desktop-launcher';
		const start = vi.fn<
			(
				program: string,
				argumentsList: readonly string[],
				workingDirectory: string,
			) => Promise<boolean>
		>(() => Promise.resolve(true));
		const started = await relaunchWindow({
			desktopRoot: '/Users/a/.local/share/orchbee/desktop',
			executablePath: '/Applications/Orchbee.app/Contents/MacOS/Orchbee',
			place,
			platform: 'darwin',
			processId: 77,
			quit: () => undefined,
			start,
			versionLauncher: launcher,
		});
		expect(started).toBe(true);
		expect(start).toHaveBeenCalledWith(
			launcher,
			[
				'--relaunch-after',
				'77',
				'--replace-application=/Applications/Orchbee.app',
				'--select-terminal=' + PRESENTATION,
			],
			'/Users/a/.local/share/orchbee/desktop',
		);
	});

	it.each([
		['reports no start', () => Promise.resolve(false)],
		['fails', () => Promise.reject(new Error('ENOENT'))],
	])('keeps the window running when starting the entry %s', async (_case, start) => {
		const quit = vi.fn<() => void>();
		const started = await relaunchWindow({
			desktopRoot: 'C:\\Orchbee\\desktop',
			executablePath: 'C:\\Orchbee\\desktop\\versions\\0.2.0\\orchbee-desktop.exe',
			place: null,
			platform: 'win32',
			processId: 77,
			quit,
			start,
			versionLauncher: null,
		});
		expect(started).toBe(false);
		expect(quit).not.toHaveBeenCalled();
	});
});
