import { execFile, spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { createConnection, type AddressInfo } from 'node:net';
import { tmpdir, userInfo } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';
import { promisify } from 'node:util';

import {
	createRsbuild,
	loadConfig,
	mergeRsbuildConfig,
	type RsbuildDevServer,
} from '@rsbuild/core';
import {
	_electron as electron,
	chromium,
	expect,
	test,
	type ElectronApplication,
	type Locator,
	type Page,
} from '@playwright/test';
import {
	decodeGatewayAttachmentClientFrame,
	decodeGatewayAttachmentServerFrame,
	decodeGatewayBrowserSearch,
	GATEWAY_CONTROL_PATH,
} from '@orchbee/gateway-protocol';

const repositoryRoot = path.resolve(import.meta.dirname, '..', '..', '..');
const hostBinary = path.join(
	repositoryRoot,
	'apps',
	'terminal-host',
	'zig-out',
	'bin',
	process.platform === 'win32' ? 'orchbee-terminal-host.exe' : 'orchbee-terminal-host',
);
const desktopMain = path.join(repositoryRoot, 'apps', 'desktop', 'dist', 'main', 'main.cjs');
const gatewayMain = path.join(repositoryRoot, 'apps', 'gateway', 'dist', 'main', 'gateway.cjs');
const serverMain = path.join(repositoryRoot, 'apps', 'server', 'dist', 'main', 'server.cjs');
const webRoot = path.join(repositoryRoot, 'apps', 'web');
const webDist = path.join(repositoryRoot, 'apps', 'web', 'dist');
const launchEnvironmentWitnessTitle = 'terminal-parity/LC-04-environment';
const automaticShellIntegrationWitnessTitle = 'terminal-parity/SI-03-automatic-shell-integration';
const sshEnvironmentWitnessTitle =
	'runs an ordinary SSH PTY with a diagnosable TERM and no implicit Orchbee environment forwarding';
const startupGeometryWitnessTitle = 'terminal-parity/SU-16-startup-geometry';
const MANUAL_NATIVE_WINDOW_TEST_TAG = '@manual-native-window';
const MANUAL_NATIVE_WINDOW_TEST_APPROVAL = 'I_ACKNOWLEDGE_FOCUS_INTERRUPTION';
const executeFile = promisify(execFile);

const applicationOutput = new WeakMap<ElectronApplication, string[]>();

let electronApplication: ElectronApplication | undefined;
let desktopEnvironment: Record<string, string> | undefined;
let desktopMainEntry: string | undefined;
let developmentRendererServer: RsbuildDevServer | undefined;
let developmentRendererUrl: string | undefined;
let hostProcess: ChildProcessWithoutNullStreams | undefined;
let rendererServer: Server | undefined;
let rendererUrl: string | undefined;
let runtimeDirectory: string | undefined;

test.beforeAll(async () => {
	const staticRenderer = await startWebRendererServer();
	rendererServer = staticRenderer.server;
	rendererUrl = staticRenderer.url;
	try {
		const developmentRenderer = await startWebRendererDevelopmentServer();
		developmentRendererServer = developmentRenderer.server;
		developmentRendererUrl = developmentRenderer.url;
	} catch (error) {
		await closeWebRendererServer(staticRenderer.server);
		rendererServer = undefined;
		rendererUrl = undefined;
		throw error;
	}
});

test.afterAll(async () => {
	try {
		await developmentRendererServer?.close();
	} finally {
		const server = rendererServer;
		if (server !== undefined) await closeWebRendererServer(server);
	}
	developmentRendererServer = undefined;
	developmentRendererUrl = undefined;
	rendererServer = undefined;
	rendererUrl = undefined;
});

test.beforeEach(async ({ browserName }, testInfo) => {
	if (browserName !== 'chromium') throw new Error('Desktop system tests require Chromium.');
	if (
		testInfo.tags.includes(MANUAL_NATIVE_WINDOW_TEST_TAG) &&
		process.env['ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL'] !==
			MANUAL_NATIVE_WINDOW_TEST_APPROVAL
	) {
		throw new Error(
			'The manual native-window test requires explicit human approval before Desktop starts.',
		);
	}
	const temporaryRoot = process.platform === 'win32' ? tmpdir() : '/tmp';
	runtimeDirectory = await mkdtemp(path.join(temporaryRoot, 'orchbee-desktop-system-'));
	desktopMainEntry = path.join(runtimeDirectory, 'desktop-system-main.cjs');
	await writeFile(
		desktopMainEntry,
		`const { app } = require('electron');\napp.setPath('home', ${JSON.stringify(runtimeDirectory)});\nrequire(${JSON.stringify(desktopMain)});\n`,
		{ mode: 0o600 },
	);
	let shell = '/bin/sh';
	let automaticShellIntegrationZdotdir: string | undefined;
	if (testInfo.title === automaticShellIntegrationWitnessTitle && process.platform === 'darwin') {
		automaticShellIntegrationZdotdir = path.join(runtimeDirectory, 'zsh-config');
		await mkdir(automaticShellIntegrationZdotdir, { mode: 0o700 });
		await Promise.all([
			writeFile(
				path.join(automaticShellIntegrationZdotdir, '.zshenv'),
				'typeset -gx ORCHBEE_SI_ZSHENV_WITNESS=preserved\n',
				{ mode: 0o600 },
			),
			writeFile(
				path.join(automaticShellIntegrationZdotdir, '.zprofile'),
				'typeset -gx ORCHBEE_SI_ZPROFILE_WITNESS=preserved\n',
				{ mode: 0o600 },
			),
			writeFile(
				path.join(automaticShellIntegrationZdotdir, '.zshrc'),
				"typeset -gx ORCHBEE_SI_ZSHRC_WITNESS=preserved\nPS1='orchbee-si03% '\n",
				{ mode: 0o600 },
			),
		]);
		shell = '/bin/zsh';
	}
	if (testInfo.title === startupGeometryWitnessTitle && process.platform !== 'win32') {
		const witnessPath = path.join(runtimeDirectory, 'startup-geometry.txt');
		const witnessShell = path.join(runtimeDirectory, 'startup-geometry-shell.sh');
		await writeFile(
			witnessShell,
			`#!/bin/sh\nstty size > ${quotePosixShellWord(witnessPath)}\nexec /bin/sh\n`,
			{ mode: 0o700 },
		);
		await chmod(witnessShell, 0o700);
		shell = witnessShell;
	}
	const endpointId = randomBytes(16).toString('hex');
	hostProcess = await startHost(runtimeDirectory, endpointId);

	const clearInheritedLocale =
		testInfo.title === launchEnvironmentWitnessTitle ||
		testInfo.title === sshEnvironmentWitnessTitle;
	const environment = Object.fromEntries(
		Object.entries(process.env).filter(
			(entry): entry is [string, string] =>
				entry[1] !== undefined &&
				(!clearInheritedLocale ||
					(entry[0] !== 'LANG' && entry[0] !== 'LC_ALL' && entry[0] !== 'LC_CTYPE')),
		),
	);
	const currentRendererUrl = rendererUrl;
	if (currentRendererUrl === undefined) throw new Error('Web renderer server is unavailable.');
	desktopEnvironment = {
		...environment,
		...(process.platform === 'win32'
			? { USERPROFILE: runtimeDirectory }
			: { HOME: runtimeDirectory }),
		ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: repositoryRoot,
		ORCHBEE_DESKTOP_ENDPOINT_ID: endpointId,
		ORCHBEE_DESKTOP_GATEWAY: '1',
		ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_RENDERER_URL: currentRendererUrl,
		ORCHBEE_DESKTOP_TRACE: '1',
		ORCHBEE_TERMINAL_HOST_BINARY: hostBinary,
		ORCHBEE_GATEWAY_ENTRY_PATH: gatewayMain,
		ORCHBEE_SERVER_ENTRY_PATH: serverMain,
		ORCHBEE_WEB_RENDERER_ROOT: webDist,
		SHELL: process.platform === 'win32' ? (environment['ComSpec'] ?? 'cmd.exe') : shell,
		...(automaticShellIntegrationZdotdir === undefined
			? {}
			: { ZDOTDIR: automaticShellIntegrationZdotdir }),
	};
	electronApplication = await launchDesktop();
});

test.afterEach(async () => {
	const runningApplication = electronApplication;
	if (runningApplication !== undefined) {
		try {
			await quitDesktopPreservingSessions(
				runningApplication,
				await runningApplication.firstWindow(),
			);
		} catch {
			await forceCloseDesktop(runningApplication);
		}
	}
	electronApplication = undefined;
	desktopEnvironment = undefined;
	desktopMainEntry = undefined;
	const runningHost = hostProcess;
	if (runningHost !== undefined && runningHost.exitCode === null) {
		runningHost.kill();
		await Promise.race([
			new Promise<void>((resolve) => runningHost.once('exit', () => resolve())),
			wait(2_000).then(() => undefined),
		]);
	}
	hostProcess = undefined;
	if (runtimeDirectory !== undefined) await rm(runtimeDirectory, { force: true, recursive: true });
	runtimeDirectory = undefined;
});

test('terminal-host/SU-17-single-process-boundary', async () => {
	test.setTimeout(45_000);
	test.skip(process.platform === 'win32', 'Windows already uses the direct named-pipe transport.');
	const application = electronApplication;
	const currentHost = hostProcess;
	const currentRuntimeDirectory = runtimeDirectory;
	if (
		application === undefined ||
		currentHost === undefined ||
		currentRuntimeDirectory === undefined
	) {
		throw new Error('Desktop single-process witness is unavailable.');
	}
	try {
		const page = await application.firstWindow();
		await expect(projectedTerminal(page)).toHaveAttribute('data-projected-terminal-state', 'open');
	} catch (error: unknown) {
		throw new Error(
			`Desktop closed before the single-process witness: ${(applicationOutput.get(application) ?? []).join('')}`,
			{ cause: error },
		);
	}
	const { stdout } = await executeFile('ps', ['-axo', 'pid=,ppid=,command='], {
		encoding: 'utf8',
	});
	const endpointProcesses = stdout
		.split('\n')
		.map((line) => line.trim())
		.filter(
			(line) =>
				line.includes(hostBinary) &&
				line.includes(`--runtime-directory ${currentRuntimeDirectory}`),
		);
	expect(endpointProcesses).toHaveLength(1);
	expect(endpointProcesses[0]).toContain(String(currentHost.pid));
	expect(endpointProcesses[0]).toContain(' serve ');
	expect(endpointProcesses[0]).not.toContain(' connect ');
	expect(endpointProcesses[0]).not.toContain(' prepare-runtime ');
});

test('terminal-parity/SU-14-runtime-presentation', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 600, width: 1_000 } });
		await page.goto(
			new URL('terminal-presentation-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-presentation-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-create-count', '1');
		await expect(root).toHaveAttribute('data-witness-open-count', '1');
		await expect(root).toHaveAttribute('data-witness-search-state', 'complete');
		await expect(root).toHaveAttribute('data-witness-search-match-count', '1');
		await expect(root).toHaveAttribute('data-witness-search-active-index', '0');
		await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
		const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');
		const initial = await runtimePresentationWitness(root);
		expect(initial.backend).toBe('canvas2d');
		expect(initial.canvasAlpha).toBe(255);
		expect(initial.inputValue).toBe('候选');
		expect(Number.parseFloat(initial.inputWidth)).toBeGreaterThan(1);
		expect(initial.presentationCommittedEpoch).toBe('0');
		expect(initial.presentationDrawnEpoch).toBe('0');

		await page.locator('#prepare-runtime-presentation').dispatchEvent('click');
		await expect(root).toHaveAttribute('data-projected-terminal-presentation-state', 'preparing');
		await expect(root).toHaveAttribute('data-witness-font-preparation-count', '4');
		await page.evaluate(
			() =>
				new Promise<void>((resolve) => {
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
				}),
		);
		const preparing = await runtimePresentationWitness(root);
		expect(preparing.renderedFrames).toBe(initial.renderedFrames);
		expect(preparing.presentationCommittedEpoch).toBe('0');
		expect(preparing.presentationDrawnEpoch).toBe('0');
		expect(preparing.canvasAlpha).toBe(initial.canvasAlpha);
		expect(preparing.canvasHeight).toBe(initial.canvasHeight);
		expect(preparing.canvasWidth).toBe(initial.canvasWidth);
		expect(preparing.inputFontFamily).toBe(initial.inputFontFamily);
		expect(preparing.inputLeft).toBe(initial.inputLeft);
		expect(preparing.inputTop).toBe(initial.inputTop);

		await page.locator('#release-runtime-font').dispatchEvent('click');
		await expect(root).toHaveAttribute('data-projected-terminal-presentation-state', 'completed');
		await expect(root).toHaveAttribute('data-projected-terminal-presentation-committed-epoch', '1');
		await expect(root).toHaveAttribute('data-projected-terminal-presentation-drawn-epoch', '1');
		const committed = await runtimePresentationWitness(root);
		expect(committed.renderedFrames).toBe(initial.renderedFrames + 1);
		expect(committed.canvasAlpha).toBe(0);
		expect(committed.canvasHeight).not.toBe(initial.canvasHeight);
		expect(committed.canvasWidth).not.toBe(initial.canvasWidth);
		expect(committed.canvasMargin).not.toBe('0px 0px 0px 0px');
		expect(committed.inputFontFamily).toBe('serif');
		expect(committed.inputFontSize).toBe('22px');
		expect(committed.inputFontWeight).toBe('500');
		expect(committed.inputLigatures).toBe('none');
		expect(committed.inputLeft).not.toBe(initial.inputLeft);
		expect(committed.inputTop).not.toBe(initial.inputTop);
		expect(committed.inputValue).toBe('候选');
		expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
		await expect(root).toHaveAttribute('data-witness-create-count', '1');
		await expect(root).toHaveAttribute('data-witness-open-count', '1');
		await expect(root).toHaveAttribute('data-witness-search-state', 'complete');
		await expect(root).toHaveAttribute('data-witness-search-match-count', '1');
		await expect(root).toHaveAttribute('data-witness-search-active-index', '0');
		await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
		await expect(root).toHaveAttribute('data-witness-selection-copy-count', '0');
		const baselineResizeCount = Number(
			await root.getAttribute('data-witness-baseline-resize-count'),
		);
		expect(Number(await root.getAttribute('data-witness-resize-count'))).toBe(
			baselineResizeCount + 1,
		);
		const inputAnchorRecovery = await root.evaluate((element) => {
			const input = element.querySelector('textarea');
			if (!(input instanceof HTMLTextAreaElement)) {
				throw new Error('Runtime presentation witness input is unavailable.');
			}
			const inputGeometry = () => ({
				height: input.style.height,
				left: input.style.left,
				lineHeight: input.style.lineHeight,
				top: input.style.top,
				width: input.style.width,
			});
			const invalidateInputGeometry = () => {
				input.style.height = '1px';
				input.style.left = '0px';
				input.style.lineHeight = '1px';
				input.style.top = '0px';
				input.style.width = '1px';
			};
			const beforeModifier = inputGeometry();
			invalidateInputGeometry();
			const keyDown = new KeyboardEvent('keydown', {
				bubbles: true,
				cancelable: true,
				code: 'ShiftLeft',
				key: 'Shift',
				shiftKey: true,
			});
			input.dispatchEvent(keyDown);
			const afterModifier = inputGeometry();
			const keyUp = new KeyboardEvent('keyup', {
				bubbles: true,
				cancelable: true,
				code: 'ShiftLeft',
				key: 'Shift',
			});
			input.dispatchEvent(keyUp);
			window.dispatchEvent(new Event('blur'));
			const beforeWindowFocus = inputGeometry();
			const activeElementRetained = document.activeElement === input;
			invalidateInputGeometry();
			window.dispatchEvent(new Event('focus'));
			return {
				activeElementRetained,
				afterModifier,
				afterWindowFocus: inputGeometry(),
				beforeModifier,
				beforeWindowFocus,
				keyDownDefaultPrevented: keyDown.defaultPrevented,
				keyUpDefaultPrevented: keyUp.defaultPrevented,
				macOs: /Mac|iPhone|iPad/u.test(navigator.platform),
			};
		});
		expect(inputAnchorRecovery.beforeModifier.width).not.toBe('1px');
		expect(inputAnchorRecovery.afterModifier).toEqual(inputAnchorRecovery.beforeModifier);
		expect(inputAnchorRecovery.activeElementRetained).toBe(true);
		expect(inputAnchorRecovery.beforeWindowFocus.width).not.toBe('1px');
		expect(inputAnchorRecovery.afterWindowFocus).toEqual(inputAnchorRecovery.beforeWindowFocus);
		expect(inputAnchorRecovery.keyDownDefaultPrevented).toBe(!inputAnchorRecovery.macOs);
		expect(inputAnchorRecovery.keyUpDefaultPrevented).toBe(!inputAnchorRecovery.macOs);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/SU-09-theme', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 600, width: 1_000 } });
		await page.goto(
			new URL('terminal-presentation-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-presentation-witness');
		const application = page.locator('html');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-projected-terminal-theme-state', 'presented');
		await expect(root).toHaveAttribute('data-projected-terminal-theme-revision', '0');
		await expect(application).toHaveAttribute('data-theme', 'light');
		const initial = await runtimeThemeWitness(root);

		await page.locator('#prepare-runtime-theme').dispatchEvent('click');
		await expect(root).toHaveAttribute('data-witness-theme-host-pending', 'true');
		await expect(root).toHaveAttribute('data-projected-terminal-theme-state', 'preparing');
		await expect(root).toHaveAttribute('data-witness-theme-update-result', 'applied-and-saved');
		await page.evaluate(
			() =>
				new Promise<void>((resolve) => {
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
				}),
		);
		const preparing = await runtimeThemeWitness(root);
		expect(preparing.applicationTheme).toBe('light');
		expect(preparing.themeRevision).toBe('0');
		expect(preparing.renderedFrames).toBe(initial.renderedFrames);
		expect(preparing.canvasRgb).toBe(initial.canvasRgb);

		await page.locator('#release-runtime-theme').dispatchEvent('click');
		await expect(application).toHaveAttribute('data-theme', 'dark');
		await expect(root).toHaveAttribute('data-projected-terminal-theme-state', 'presented');
		await expect(root).toHaveAttribute('data-projected-terminal-theme-revision', '1');
		await expect(root).toHaveAttribute('data-projected-terminal-rendered-revision', '2');
		await expect(root).not.toHaveAttribute('data-witness-theme-host-pending');
		const committed = await runtimeThemeWitness(root);
		expect(committed.applicationTheme).toBe('dark');
		expect(committed.themeRevision).toBe('1');
		expect(committed.renderedFrames).toBe(initial.renderedFrames + 1);
		expect(committed.canvasRgb).not.toBe(initial.canvasRgb);
		await expect(root).toHaveAttribute('data-witness-create-count', '1');
		await expect(root).toHaveAttribute('data-witness-open-count', '1');
	} finally {
		await browser.close();
	}
});

test('terminal-parity/SU-13-cursor-defaults', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-shape', 'block');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-blinking', 'false');

	const input = page.getByRole('textbox');
	await input.pressSequentially(`printf '\\033[5 q'`, { delay: 1 });
	await input.press('Enter');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-shape', 'bar');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-blinking', 'true');

	await input.pressSequentially(`printf '\\033c'`, { delay: 1 });
	await input.press('Enter');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-shape', 'block');
	await expect(root).toHaveAttribute('data-projected-terminal-cursor-blinking', 'false');
});

test(startupGeometryWitnessTitle, async () => {
	test.skip(process.platform === 'win32', 'The exact startup witness uses POSIX stty.');
	const application = electronApplication;
	const currentRuntimeDirectory = runtimeDirectory;
	if (application === undefined) throw new Error('Electron application did not start.');
	if (currentRuntimeDirectory === undefined) {
		throw new Error('Desktop runtime directory is unavailable.');
	}
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(root).toHaveAttribute('data-projected-terminal-rendered-frames', '1');

	const firstClientGeometry = await root.evaluate((element) => {
		if (!(element instanceof HTMLElement)) throw new Error('Terminal root is unavailable.');
		const canvas = element.querySelector('canvas');
		if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Terminal canvas is unavailable.');
		const numberAttribute = (name: string): number =>
			Number(element.getAttribute(name) ?? Number.NaN);
		const columns = numberAttribute('data-projected-terminal-columns');
		const rows = numberAttribute('data-projected-terminal-rows');
		const canvasWidth = Number.parseFloat(canvas.style.width);
		const canvasHeight = Number.parseFloat(canvas.style.height);
		const style = getComputedStyle(element);
		const horizontalPadding =
			Number.parseFloat(style.paddingLeft) + Number.parseFloat(style.paddingRight);
		const verticalPadding =
			Number.parseFloat(style.paddingTop) + Number.parseFloat(style.paddingBottom);
		const cellCssWidth = canvasWidth / columns;
		const cellCssHeight = canvasHeight / rows;
		return {
			cellPixelHeight: numberAttribute('data-projected-terminal-cell-pixel-height'),
			cellPixelWidth: numberAttribute('data-projected-terminal-cell-pixel-width'),
			columns,
			expectedCellPixelHeight: Math.round(cellCssHeight * window.devicePixelRatio),
			expectedCellPixelWidth: Math.round(cellCssWidth * window.devicePixelRatio),
			expectedColumns: Math.floor((element.clientWidth - horizontalPadding) / cellCssWidth),
			expectedRows: Math.floor((element.clientHeight - verticalPadding) / cellCssHeight),
			rows,
		};
	});
	expect(firstClientGeometry).toMatchObject({
		cellPixelHeight: firstClientGeometry.expectedCellPixelHeight,
		cellPixelWidth: firstClientGeometry.expectedCellPixelWidth,
		columns: firstClientGeometry.expectedColumns,
		rows: firstClientGeometry.expectedRows,
	});
	expect({ columns: firstClientGeometry.columns, rows: firstClientGeometry.rows }).not.toEqual({
		columns: 80,
		rows: 24,
	});

	const witnessPath = path.join(currentRuntimeDirectory, 'startup-geometry.txt');
	await expect
		.poll(() =>
			readFile(witnessPath, 'utf8')
				.then((value) => value.trim())
				.catch(() => ''),
		)
		.toMatch(/^\d+ \d+$/u);
	const [initialPtyRows, initialPtyColumns] = (await readFile(witnessPath, 'utf8'))
		.trim()
		.split(/\s+/u)
		.map(Number);
	expect({ columns: initialPtyColumns, rows: initialPtyRows }).toEqual({
		columns: firstClientGeometry.columns,
		rows: firstClientGeometry.rows,
	});
});

test('terminal-parity/SU-15-visibility', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch({ headless: false });
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-visibility-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-visibility-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-renderer', 'canvas2d');
		await expect(root).toHaveAttribute(
			'data-witness-control',
			'synthetic-application-layer-document-visibility',
		);
		await expect(root).toHaveAttribute(
			'data-witness-limitation',
			'does-not-exercise-os-tab-background-throttling',
		);
		await expect(root).toHaveAttribute('data-witness-initial-render-count', '1');
		await expect(root).toHaveAttribute('data-witness-initial-acknowledgement-count', '1');
		await expect(root).toHaveAttribute('data-witness-initial-generation', '1');
		await expect(root).toHaveAttribute('data-witness-initial-revision', '1');
		await expect(root).toHaveAttribute('data-witness-joint-pause-complete', 'true');
		await expect(root).toHaveAttribute('data-witness-joint-pause-committed-count', '3');
		await expect(root).toHaveAttribute('data-witness-joint-pause-acknowledgement-count', '4');
		await expect(root).toHaveAttribute('data-witness-joint-pause-render-delta', '0');
		await expect(root).toHaveAttribute('data-witness-joint-pause-generation', '2');
		await expect(root).toHaveAttribute('data-witness-joint-pause-revision', '4');
		expect(
			Number(await root.getAttribute('data-witness-joint-pause-animation-window-ms')),
		).toBeGreaterThan(530);

		await page.locator('#clear-visibility-pause-cause').click();
		await expect(root).toHaveAttribute('data-witness-clear-one-complete', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-clear-one-render-delta', '0');
		expect(
			Number(await root.getAttribute('data-witness-clear-one-animation-window-ms')),
		).toBeGreaterThan(530);

		await page.locator('#clear-demand-pause-cause').click();
		await expect(root).toHaveAttribute('data-witness-complete', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-resume-complete', 'true');
		await expect(root).toHaveAttribute('data-witness-resume-render-delta', '1');
		await expect(root).toHaveAttribute('data-witness-resume-generation', '2');
		await expect(root).toHaveAttribute('data-witness-resume-revision', '4');
		await expect(root).toHaveAttribute('data-witness-second-render-delta', '0');
		await expect(root).toHaveAttribute('data-witness-dispose-child-count', '0');
		await expect(root).toHaveAttribute('data-witness-dispose-command-delta', '0');
		await expect(root).toHaveAttribute('data-witness-dispose-render-delta', '0');
		await expect(root).toHaveAttribute('data-witness-post-dispose-apply-result', 'disposed');
		expect(
			Number(await root.getAttribute('data-witness-dispose-animation-window-ms')),
		).toBeGreaterThan(530);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/SU-08-dpr', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch({ headless: false });
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(new URL('terminal-dpr-fixture.html', currentDevelopmentRendererUrl).href);
		const root = page.locator('#terminal-dpr-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-control',
			'cdp-device-metrics-with-browser-resize',
		);
		await expect(root).toHaveAttribute(
			'data-witness-limitation',
			'synthetic-chromium-dpr-transition-not-native-monitor-transfer',
		);
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		const initial = await dprWitness(root);
		expect(initial.devicePixelRatio).toBe(1);
		expect(initial.columns).toBe(24);
		expect(initial.rows).toBe(4);
		expect(initial.canvasWidth).toBe(initial.canvasCssWidth);
		expect(initial.canvasHeight).toBe(initial.canvasCssHeight);
		expect(initial.atlasClearCount).toBe(1);
		expect(initial.renderCount).toBe(1);
		expect(initial.resizeCount).toBeGreaterThan(0);
		expect(initial.resizeColumns).toBeGreaterThan(0);
		expect(initial.resizeRows).toBeGreaterThan(0);
		expect(initial.resizeTerminalPixelWidth).toBe(
			initial.resizeCellPixelWidth * initial.resizeColumns,
		);
		expect(initial.resizeTerminalPixelHeight).toBe(
			initial.resizeCellPixelHeight * initial.resizeRows,
		);
		expect(initial.matchMediaCallCount).toBe(1);
		expect(initial.matchMediaActiveCount).toBe(1);
		expect(initial.windowResizeActiveCount).toBe(1);

		const cdp = await page.context().newCDPSession(page);
		await cdp.send('Emulation.setDeviceMetricsOverride', {
			deviceScaleFactor: 1.5,
			height: 500,
			mobile: false,
			width: 901,
		});
		await expect(root).toHaveAttribute('data-witness-dpr-transition', 'complete');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		const transitioned = await dprWitness(root);
		expect(transitioned.devicePixelRatio).toBe(1.5);
		expect(transitioned.columns).toBe(initial.columns);
		expect(transitioned.rows).toBe(initial.rows);
		expect(transitioned.canvasCssWidth).toBe(initial.canvasCssWidth);
		expect(transitioned.canvasCssHeight).toBe(initial.canvasCssHeight);
		expect(transitioned.canvasWidth).toBe(initial.canvasCssWidth * 1.5);
		expect(transitioned.canvasHeight).toBe(initial.canvasCssHeight * 1.5);
		expect(transitioned.canvasAlpha).toBe(255);
		expect(transitioned.inputGeometry).toBe(initial.inputGeometry);
		expect(transitioned.inputAnchorCoherent).toBe(true);
		expect(transitioned.atlasClearCount).toBe(initial.atlasClearCount + 1);
		expect(transitioned.atlasClearDelta).toBe(1);
		expect(transitioned.canvasFillTextDelta).toBeGreaterThan(0);
		expect(transitioned.renderCount).toBe(initial.renderCount + 1);
		expect(transitioned.renderDelta).toBe(1);
		expect(transitioned.lastForceAll).toBe(true);
		expect(transitioned.lastReasons).toContain('surface');
		expect(transitioned.lastRowsRendered).toBe(4);
		expect(transitioned.lastCellsVisited).toBe(96);
		expect(transitioned.lastGeneration).toBe('1');
		expect(transitioned.lastRevision).toBe('1');
		expect(transitioned.resizeCount).toBe(initial.resizeCount + 1);
		expect(transitioned.resizeDelta).toBe(1);
		expect(transitioned.resizeColumns).toBe(initial.resizeColumns);
		expect(transitioned.resizeRows).toBe(initial.resizeRows);
		expect(transitioned.resizeCellPixelWidth).toBe(
			Math.round((initial.canvasCssWidth / initial.columns) * 1.5),
		);
		expect(transitioned.resizeCellPixelHeight).toBe(
			Math.round((initial.canvasCssHeight / initial.rows) * 1.5),
		);
		expect(transitioned.resizeTerminalPixelWidth).toBe(
			transitioned.resizeCellPixelWidth * transitioned.resizeColumns,
		);
		expect(transitioned.resizeTerminalPixelHeight).toBe(
			transitioned.resizeCellPixelHeight * transitioned.resizeRows,
		);
		expect(transitioned.matchMediaCallCount).toBe(2);
		expect(transitioned.matchMediaActiveCount).toBe(1);
		expect(transitioned.matchMediaRemoveCount).toBe(1);
		expect(transitioned.matchMediaLastQuery).toBe('(resolution: 1.5dppx)');
		expect(transitioned.windowResizeInvocationCount).toBeGreaterThanOrEqual(1);

		await page.locator('#dispose-dpr-witness').click();
		await expect(root).toHaveAttribute('data-witness-dispose-complete', 'true');
		await expect(root).toHaveAttribute('data-witness-dispose-child-count', '0');
		const disposed = await dprWitness(root);
		expect(disposed.matchMediaActiveCount).toBe(0);
		expect(disposed.windowResizeActiveCount).toBe(0);

		await cdp.send('Emulation.setDeviceMetricsOverride', {
			deviceScaleFactor: 2,
			height: 500,
			mobile: false,
			width: 902,
		});
		await page.waitForFunction(() => window.devicePixelRatio === 2 && window.innerWidth === 902);
		await page.evaluate(
			() =>
				new Promise<void>((resolve) => {
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
				}),
		);
		const afterDispose = await dprWitness(root);
		expect(afterDispose.terminalMutationCount).toBe(disposed.terminalMutationCount);
		expect(afterDispose.renderCount).toBe(disposed.renderCount);
		expect(afterDispose.resizeCount).toBe(disposed.resizeCount);
		expect(afterDispose.matchMediaCallCount).toBe(disposed.matchMediaCallCount);
		expect(afterDispose.matchMediaInvocationCount).toBe(disposed.matchMediaInvocationCount);
		expect(afterDispose.matchMediaActiveCount).toBe(0);
		expect(afterDispose.windowResizeActiveCount).toBe(0);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/LC-07-resize-lease', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 600, width: 900 } });
		await page.goto(
			new URL('terminal-resize-ownership-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-resize-ownership-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute(
			'data-witness-control',
			'chromium-resize-observer-with-explicit-lease-grant',
		);
		await expect(root).toHaveAttribute(
			'data-witness-limitation',
			'client-convergence-only-not-a-physical-phone',
		);
		await expect(root).toHaveAttribute('data-witness-observer-width', '320');
		await expect(root).toHaveAttribute('data-witness-observer-height', '480');
		await expect(root).toHaveAttribute('data-witness-resize-count', '0');

		await page.locator('#grant-resize-ownership').click();
		await expect(root).toHaveAttribute('data-witness-complete', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-resize-count', '1');
		expect(Number(await root.getAttribute('data-witness-columns'))).toBeGreaterThan(0);
		expect(Number(await root.getAttribute('data-witness-rows'))).toBeGreaterThan(0);
		expect(
			Number(await root.getAttribute('data-witness-terminal-pixel-width')),
		).toBeLessThanOrEqual(320);
		expect(
			Number(await root.getAttribute('data-witness-terminal-pixel-height')),
		).toBeLessThanOrEqual(480);

		await page.locator('#dispose-resize-ownership-witness').click();
		await expect(root).toHaveAttribute('data-witness-disposed', 'true');
		await expect(root).toHaveAttribute('data-witness-dispose-child-count', '0');
	} finally {
		await browser.close();
	}
});

test('terminal-parity/CF-03-visual-environment', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch({ headless: false });
	try {
		const context = await browser.newContext({
			colorScheme: 'light',
			deviceScaleFactor: 1,
			locale: 'en-US',
			reducedMotion: 'reduce',
			timezoneId: 'UTC',
			viewport: { height: 500, width: 900 },
		});
		const page = await context.newPage();
		await page.goto(
			new URL('terminal-presentation-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-presentation-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-projected-terminal-renderer', 'canvas2d');
		const environment = await page.evaluate(async () => {
			await document.fonts.ready;
			const fontFaces = await document.fonts.load('400 14px "Orchbee JetBrains Mono"', 'MW');
			const input = document.querySelector('textarea');
			return {
				colorScheme: matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'other',
				devicePixelRatio: window.devicePixelRatio,
				direction: document.documentElement.dir,
				fontFaces: fontFaces.map((face) => ({ family: face.family, status: face.status })),
				inputFontFamily: input instanceof HTMLTextAreaElement ? input.style.fontFamily : '',
				language: document.documentElement.lang,
				locale: navigator.language,
				reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches,
				timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
				viewport: { height: window.innerHeight, width: window.innerWidth },
			};
		});
		expect(environment).toMatchObject({
			colorScheme: 'light',
			devicePixelRatio: 1,
			direction: 'ltr',
			language: 'en-US',
			locale: 'en-US',
			reducedMotion: true,
			timezone: 'UTC',
			viewport: { height: 500, width: 900 },
		});
		expect(environment.fontFaces).toEqual([
			expect.objectContaining({
				family: expect.stringContaining('Orchbee JetBrains Mono'),
				status: 'loaded',
			}),
		]);
		expect(environment.inputFontFamily).toContain('Orchbee JetBrains Mono');
	} finally {
		await browser.close();
	}
});

test('terminal-parity/IN-02-composition', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-input-repair-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-input-repair-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-renderer', 'canvas2d');
		await expect(root).toHaveAttribute(
			'data-witness-synthetic-event-scope',
			'application-layer-only',
		);
		await expect(root).toHaveAttribute('data-witness-create-count', '1');
		await expect(root).toHaveAttribute('data-witness-open-count', '1');
		await expect(root).toHaveAttribute('data-witness-active-preedit-observed', 'true');
		await expect(root).toHaveAttribute('data-witness-repair-draw-delta', '1');
		await expect(root).toHaveAttribute('data-witness-late-update-draw-delta', '0');
		await expect(root).toHaveAttribute('data-witness-late-end-text-count', '0');
		await expect(root).toHaveAttribute('data-witness-repeated-repair-guard-text-count', '0');
		await expect(root).toHaveAttribute('data-witness-new-composition-text-count', '1');
		await expect(root).toHaveAttribute('data-witness-restart-before-old-end-text-count', '1');
		await expect(root).toHaveAttribute('data-witness-duplicate-end-text-count', '1');
		await expect(root).toHaveAttribute('data-witness-shifted-native-text', '：');
		await expect(root).toHaveAttribute('data-witness-shifted-native-key-count', '0');
		await expect(root).toHaveAttribute('data-witness-shifted-fallback-text', ':');
		await expect(root).toHaveAttribute('data-witness-shifted-fallback-key-count', '0');
		await expect(root).toHaveAttribute('data-witness-textarea-cleared', 'true');
		await expect(root).toHaveAttribute('data-witness-same-attachment', 'true');
		await expect(root).toHaveAttribute('data-witness-dom-child-count-after-dispose', '0');
		const attachment = await root.getAttribute('data-witness-attachment-id');
		expect(attachment).toMatch(/^[0-9a-f]{32}$/u);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/PR-11-bell', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 1000 } });
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await page.goto(
			new URL('terminal-bell-effects-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-bell-effects-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-same-terminal', 'true');
		await expect(root).toHaveAttribute('data-witness-duplicate-surface-inactive', 'true');
		await expect(root).toHaveAttribute('data-witness-reconnect-replay-inactive', 'true');
		await expect(root).toHaveAttribute('data-witness-effect-sequences', '6,7,8');
		await expect(root).toHaveAttribute('data-witness-sound-effects', '5,7,6,8');
		await expect(root).toHaveAttribute('data-witness-notification-effects', '5,7,6,8');
		await expect(root).toHaveAttribute('data-witness-sound-failure-isolated', 'true');
		await expect(root).toHaveAttribute('data-witness-visual-phases', 'a,b');
		await expect(root).toHaveAttribute('data-witness-visual-surface-border-free', 'true');
		await expect(root).toHaveAttribute('data-witness-visual-surface-pointer-transparent', 'true');
		await expect(root).toHaveAttribute('data-witness-visual-surface-visible', 'true');
	} finally {
		await browser.close();
	}
});

test('terminal-parity/IN-10-read-only', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-input-capability-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-input-capability-witness');
		await expect(root).toHaveAttribute('data-witness-step', 'fresh-input-observed');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-read-only-observed', 'true');
		await expect(root).toHaveAttribute('data-witness-upgrade-observed', 'true');
		await expect(root).toHaveAttribute('data-witness-paste-review-aborted', 'true');
		await expect(root).toHaveAttribute('data-witness-textarea-cleared', 'true');
		await expect(root).toHaveAttribute('data-witness-search-available', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-local-operations',
			'copy,history,search,selection',
		);
		await expect(root).toHaveAttribute('data-witness-downgrade-input-command-delta', '0');
		await expect(root).toHaveAttribute('data-witness-stale-paste-count', '0');
		await expect(root).toHaveAttribute('data-witness-stale-text-count', '0');
		await expect(root).toHaveAttribute('data-witness-fresh-input-revisions', '3,3');
		await expect(root).toHaveAttribute('data-projected-terminal-capability-revision', '3');
		await expect(root).toHaveAttribute('data-projected-terminal-input-capability', 'read-write');
	} finally {
		await browser.close();
	}
});

test('terminal-parity/FS-18-panel-broadcast-input', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 1000 } });
		await page.goto(
			new URL('panel-broadcast-input-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#panel-broadcast-input-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		const viewport = page.locator('[data-slot="workbench-terminal-viewport"]').last();
		await viewport.focus();
		await viewport.click({ button: 'right', position: { x: 30, y: 30 } });
		await page.getByRole('menuitem', { name: 'Broadcast Input' }).click();
		const input = page.getByRole('textbox', { name: 'Broadcast input' });
		await expect(input).toBeVisible();
		await expect(page.locator('[data-panel-broadcast-target="true"]')).toHaveCount(3);

		await input.evaluate((element) => {
			if (!(element instanceof HTMLInputElement)) throw new Error('Broadcast input is missing.');
			element.value = 'forbidden-paste';
			element.dispatchEvent(
				new InputEvent('input', {
					bubbles: true,
					data: 'forbidden-paste',
					inputType: 'insertFromPaste',
				}),
			);
			element.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
			element.value = '部署';
			element.dispatchEvent(
				new InputEvent('input', {
					bubbles: true,
					data: '部署',
					inputType: 'insertCompositionText',
					isComposing: true,
				}),
			);
			element.dispatchEvent(
				new CompositionEvent('compositionend', { bubbles: true, data: '部署' }),
			);
		});
		await expect(root).toHaveAttribute(
			'data-witness-deliveries',
			'1:text:部署|2:text:部署|3:text:部署',
		);
		await input.evaluate((element) => {
			if (!(element instanceof HTMLInputElement)) throw new Error('Broadcast input is missing.');
			for (const text of ['first', 'second']) {
				element.value = text;
				element.dispatchEvent(
					new InputEvent('input', {
						bubbles: true,
						data: text,
						inputType: 'insertText',
					}),
				);
			}
		});
		await expect(root).toHaveAttribute(
			'data-witness-deliveries',
			'1:text:部署>text:first>text:second|2:text:部署>text:first>text:second|3:text:部署>text:first>text:second',
		);
		await input.press('Escape');
		await expect(root).toHaveAttribute(
			'data-witness-deliveries',
			'1:text:部署>text:first>text:second>key:Escape|2:text:部署>text:first>text:second>key:Escape|3:text:部署>text:first>text:second>key:Escape',
		);
		await input.press('Control+Alt+Escape');
		const hiddenInput = page.locator('[data-slot="workbench-panel-broadcast-input-field"]');
		await expect(hiddenInput).toHaveAttribute('type', 'password');
		await hiddenInput.press('Control+Escape');
		await expect(input).toHaveCount(0);
		await expect(page.locator('[data-panel-broadcast-target="true"]')).toHaveCount(0);

		await viewport.focus();
		await viewport.click({ button: 'right', position: { x: 30, y: 30 } });
		await page.getByRole('menuitem', { name: 'Broadcast Input' }).click();
		await page.getByRole('textbox', { name: 'Broadcast input' }).fill('partial');
		const warning = page.getByRole('alert');
		await expect(warning).toContainText('confirmed by 2 of 3 terminals');
		await expect(page.getByRole('textbox', { name: 'Broadcast input' })).toHaveCount(0);
		await expect(root).toHaveAttribute(
			'data-witness-deliveries',
			'1:text:部署>text:first>text:second>key:Escape>text:partial|2:text:部署>text:first>text:second>key:Escape>text:partial|3:text:部署>text:first>text:second>key:Escape>text:partial',
		);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/IN-09-action-scope', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-workbench-scope', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-apple-commands',
			'unbound-native:true,unavailable-native:true,available-claimed:true,clear-claimed:true',
		);
		await expect(root).toHaveAttribute('data-witness-confirmation-count', '0');
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/IN-07-pre-input', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-buttonless-motion-report-count', '1');
		await expect(root).toHaveAttribute('data-witness-buttonless-motion-selection-clear-count', '0');
		await expect(root).toHaveAttribute('data-witness-buttonless-motion-history-bottom-count', '0');
		await expect(root).toHaveAttribute(
			'data-witness-buttonless-motion-selection-preserved',
			'true',
		);
		await expect(root).toHaveAttribute('data-witness-focus-loss-selection-clear-count', '0');
		await expect(root).toHaveAttribute('data-witness-focus-loss-selection-preserved', 'true');
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/IN-11-context', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-application-mouse-owned', 'true');
		await expect(root).toHaveAttribute('data-witness-pinned-context', 'true');
		await expect(root).toHaveAttribute('data-witness-action-inputs', 'terminal.link.open');
		await expect(root).toHaveAttribute('data-witness-external-open-count', '0');
		await expect(root).toHaveAttribute(
			'data-witness-typed-results',
			'paste:completed,scroll:unchanged,clear:completed,stale-link:blocked:stale-terminal-link-context',
		);
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/IN-12-actions', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute(
			'data-witness-catalog',
			[
				'terminal.selection.copy',
				'terminal.clipboard.paste',
				'terminal.selection.select-all',
				'terminal.search.open',
				'terminal.search.next',
				'terminal.search.previous',
				'terminal.output.show-last',
				'terminal.output.copy',
				'terminal.history.show',
				'terminal.history.copy',
				'terminal.history.prompt-previous',
				'terminal.history.prompt-next',
				'terminal.history.scroll-line-up',
				'terminal.history.scroll-line-down',
				'terminal.history.scroll-page-up',
				'terminal.history.scroll-page-down',
				'terminal.history.scroll-top',
				'terminal.history.scroll-bottom',
				'terminal.clear',
				'terminal.reset',
			].join(','),
		);
		await expect(root).toHaveAttribute(
			'data-witness-availability',
			[
				'terminal.selection.copy:disabled',
				'terminal.clipboard.paste:enabled',
				'terminal.selection.select-all:enabled',
				'terminal.search.open:enabled',
				'terminal.search.next:disabled',
				'terminal.search.previous:disabled',
				'terminal.output.show-last:enabled',
				'terminal.output.copy:disabled',
				'terminal.history.show:enabled',
				'terminal.history.copy:disabled',
				'terminal.history.prompt-previous:enabled',
				'terminal.history.prompt-next:enabled',
				'terminal.history.scroll-line-up:enabled',
				'terminal.history.scroll-line-down:enabled',
				'terminal.history.scroll-page-up:enabled',
				'terminal.history.scroll-page-down:enabled',
				'terminal.history.scroll-top:enabled',
				'terminal.history.scroll-bottom:enabled',
				'terminal.clear:enabled',
				'terminal.reset:enabled',
			].join(','),
		);
		await expect(root).toHaveAttribute(
			'data-witness-feedback',
			'已将剪贴板文本粘贴到终端。|已清除终端输出。|终端链接已发生变化，无法打开。',
		);
		await expect(root).toHaveAttribute('data-witness-confirmation-count', '0');
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/FS-08-command-output-viewer', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).toHaveAttribute('data-witness-output-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		const input = page.getByRole('textbox', { name: '终端输入' });
		await input.focus();
		const shortcutKeyInputCount = await root.getAttribute(
			'data-witness-output-shortcut-key-input-count',
		);
		await page.keyboard.press('Control+Shift+KeyG');

		const viewer = page.getByRole('dialog', { name: '上一条命令输出' });
		await expect(viewer).toBeVisible();
		await expect(viewer.locator('pre')).toHaveText('history row 8\nhistory row 9');
		expect(await root.getAttribute('data-witness-output-shortcut-key-input-count')).toBe(
			shortcutKeyInputCount,
		);

		const query = page.getByRole('textbox', { name: '在命令输出中查找' });
		await expect(query).toBeFocused();
		await query.fill('row');
		const matches = viewer.locator('mark');
		await expect(matches).toHaveCount(2);
		await expect(matches.nth(0)).toHaveAttribute('data-active', 'true');
		await page.getByRole('button', { name: '查找下一个匹配项' }).click();
		await expect(matches.nth(1)).toHaveAttribute('data-active', 'true');

		await page.getByRole('button', { name: '复制命令输出' }).click();
		await expect(root).toHaveAttribute('data-witness-output-clipboard-write-count', '1');
		await expect(root).toHaveAttribute(
			'data-witness-output-copied-text',
			'history row 8\nhistory row 9',
		);
		await expect(root.locator('[data-projected-terminal-status="true"]')).toHaveText(
			'已复制命令输出。',
		);

		await page.keyboard.press('Escape');
		await expect(viewer).toHaveCount(0);
		await expect(input).toBeFocused();
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/FS-09-scrollback-viewer', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalActionsWitness();
	try {
		const root = page.locator('#terminal-actions-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).toHaveAttribute('data-witness-scrollback-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-projected-terminal-attachment-id', '55'.repeat(16));
		const input = page.getByRole('textbox', { name: '终端输入' });
		await input.focus();
		const shortcutKeyInputCount = await root.getAttribute(
			'data-witness-scrollback-shortcut-key-input-count',
		);
		await page.keyboard.press('Control+Shift+KeyH');

		const viewer = page.getByRole('dialog', { name: '回滚缓冲区' });
		await expect(viewer).toBeVisible();
		const scrollbackText = [
			...Array.from({ length: 10 }, (_, index) => `history row ${index}`),
			'linked context',
			'terminal actions',
			'localized feedback',
			'native command',
			'visible row 4',
			'visible row 5',
			'visible row 6',
			'visible row 7',
			'visible row 8',
			'visible row 9',
		].join('\n');
		await expect(viewer.locator('pre')).toHaveText(scrollbackText);
		expect(await root.getAttribute('data-witness-scrollback-shortcut-key-input-count')).toBe(
			shortcutKeyInputCount,
		);

		const content = page.getByRole('region', { name: '保留的终端回滚缓冲区' });
		const anchor = content.locator('[data-initial-scrollback-anchor="true"]');
		await expect(anchor).toHaveCount(1);
		expect(
			await anchor.evaluate((element) =>
				element.nextSibling?.textContent?.startsWith('linked context'),
			),
		).toBe(true);
		const scrollGeometry = await content.evaluate((element) => {
			const anchorElement = element.querySelector<HTMLElement>(
				'[data-initial-scrollback-anchor="true"]',
			);
			return {
				anchorTop: anchorElement?.getBoundingClientRect().top ?? null,
				clientHeight: element.clientHeight,
				contentTop: element.getBoundingClientRect().top,
				scrollHeight: element.scrollHeight,
				scrollTop: element.scrollTop,
			};
		});
		if (scrollGeometry.anchorTop === null) throw new Error('FS-09 scrollback anchor is missing.');
		expect(scrollGeometry.scrollHeight).toBeGreaterThan(scrollGeometry.clientHeight);
		expect(scrollGeometry.scrollTop).toBeGreaterThan(0);
		expect(scrollGeometry.anchorTop).toBeGreaterThanOrEqual(scrollGeometry.contentTop);
		expect(scrollGeometry.anchorTop - scrollGeometry.contentTop).toBeLessThan(32);

		const query = page.getByRole('textbox', { name: '在回滚缓冲区中查找' });
		await expect(query).toBeFocused();
		await query.fill('history row');
		const matches = viewer.locator('mark');
		await expect(matches).toHaveCount(10);
		await expect(matches.nth(0)).toHaveAttribute('data-active', 'true');
		await page.getByRole('button', { name: '查找下一个匹配项' }).click();
		await expect(matches.nth(1)).toHaveAttribute('data-active', 'true');

		await page.getByRole('button', { name: '复制回滚缓冲区' }).click();
		await expect(root).toHaveAttribute('data-witness-scrollback-clipboard-write-count', '1');
		await expect(root).toHaveAttribute('data-witness-scrollback-copied-text', scrollbackText);
		await expect(root.locator('[data-projected-terminal-status="true"]')).toHaveText(
			'已复制回滚缓冲区。',
		);

		await page.keyboard.press('Escape');
		await expect(viewer).toHaveCount(0);
		await expect(input).toBeFocused();
		await expect(root).toHaveAttribute('data-projected-terminal-attachment-id', '55'.repeat(16));
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/FS-05-file-links', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalUrlPolicyWitness();
	try {
		const root = page.locator('#terminal-url-policy-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-direct-file-effect',
			'failed:link-policy-blocked',
		);
		for (const attribute of [
			'data-witness-blocked-hover',
			'data-witness-blocked-context',
			'data-witness-blocked-activation',
		]) {
			await expect(root).toHaveAttribute(
				attribute,
				'auto-ftp,explicit-ftp,explicit-file,detected-path',
			);
		}
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/FS-06-url-policy', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalUrlPolicyWitness();
	try {
		const root = page.locator('#terminal-url-policy-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		for (const attribute of [
			'data-witness-allowed-hover',
			'data-witness-allowed-context',
			'data-witness-allowed-activation',
		]) {
			await expect(root).toHaveAttribute(attribute, 'http,https,mailto');
		}
		await expect(root).toHaveAttribute(
			'data-witness-opened-targets',
			[
				'http://orchbee.dev/http',
				'http://orchbee.dev/http',
				'https://orchbee.dev/secure',
				'https://orchbee.dev/secure',
				'mailto:terminal@orchbee.dev',
				'mailto:terminal@orchbee.dev',
			].join('|'),
		);
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/FS-07-keyboard-hints', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalUrlPolicyWitness();
	try {
		const root = page.locator('#terminal-url-policy-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).toHaveAttribute('data-witness-hints-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		const input = page.getByRole('textbox', { name: 'Terminal input' });
		await input.focus();
		await page.keyboard.press('Meta+Shift+KeyO');
		const hints = root.locator('[data-projected-terminal-hint="true"]');
		await expect(hints).toHaveCount(4);
		expect(
			await hints.evaluateAll((elements) =>
				elements.map((element) => element.getAttribute('data-projected-terminal-hint-kind')),
			),
		).toEqual(['url', 'path', 'file-line', 'hash']);

		const hashHint = root.locator('[data-projected-terminal-hint-kind="hash"]');
		const resolvedHashLabel = await hashHint.getAttribute('data-projected-terminal-hint-label');
		if (resolvedHashLabel === null) throw new Error('FS-07 hash hint has no keyboard label.');
		await page.keyboard.press(resolvedHashLabel);
		await expect(root).toHaveAttribute('data-witness-hint-insertions', 'deadbee');
		await expect(root).toHaveAttribute('data-witness-hint-key-inputs', '0');
		await expect(hints).toHaveCount(0);

		await page.keyboard.press('Meta+Shift+KeyO');
		await expect(hints).toHaveCount(4);
		const urlHint = root.locator('[data-projected-terminal-hint-kind="url"]');
		const urlLabel = await urlHint.getAttribute('data-projected-terminal-hint-label');
		if (urlLabel === null) throw new Error('FS-07 URL hint has no keyboard label.');
		await page.keyboard.press(urlLabel);
		await expect(root).toHaveAttribute('data-witness-opened-targets', /\|https:\/\/o\.dev\/?$/u);
		await expect(root).toHaveAttribute('data-witness-hint-key-inputs', '0');
		await expect(hints).toHaveCount(0);

		await page.keyboard.press('Meta+Shift+KeyO');
		await expect(hints).toHaveCount(4);
		await page.keyboard.press('Escape');
		await expect(hints).toHaveCount(0);
		await expect(root).toHaveAttribute('data-witness-hint-insertions', 'deadbee');
		await expect(root).toHaveAttribute('data-witness-hint-key-inputs', '0');
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/LC-05-lan-capability', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalClipboardCapabilityWitness();
	try {
		const root = page.locator('#terminal-clipboard-capability-witness');
		await expect(root).toHaveAttribute('data-witness-setup-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-actual-secure-context', 'true');
		await expect(root).toHaveAttribute('data-witness-initial-user-activation', 'false');
		await expect(root).toHaveAttribute('data-witness-initial-availability', 'disabled');
		await expect(root).toHaveAttribute('data-witness-initial-result', 'blocked:action-unavailable');
		await expect(root).toHaveAttribute(
			'data-witness-initial-feedback',
			'This terminal action is unavailable.',
		);
		await expect(root).toHaveAttribute('data-witness-initial-selection-commands', '0');
		await expect(root).toHaveAttribute('data-witness-initial-writes', '0');
		await expect(root).toHaveAttribute('data-witness-read-capability', 'false');

		await page.locator('#terminal-clipboard-capability-activation').click();
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-click-user-activation', 'true');
		await expect(root).toHaveAttribute('data-witness-activated-availability', 'enabled');
		await expect(root).toHaveAttribute('data-witness-activated-result', 'completed');
		await expect(root).toHaveAttribute('data-witness-activated-feedback', 'Copied 21 bytes.');
		await expect(root).toHaveAttribute('data-witness-activated-selection-commands', '1');
		await expect(root).toHaveAttribute('data-witness-activated-writes', '1');
		await expect(root).toHaveAttribute(
			'data-witness-rejected-result',
			'failed:browser-clipboard-write-failed',
		);
		await expect(root).toHaveAttribute(
			'data-witness-rejected-feedback',
			'The terminal selection could not be copied.',
		);
		await expect(root).toHaveAttribute('data-witness-rejected-command-delta', '1');
		await expect(root).toHaveAttribute('data-witness-rejected-write-delta', '1');
		await expect(root).toHaveAttribute('data-witness-missing-availability', 'disabled');
		await expect(root).toHaveAttribute('data-witness-missing-result', 'blocked:action-unavailable');
		await expect(root).toHaveAttribute(
			'data-witness-missing-feedback',
			'This terminal action is unavailable.',
		);
		await expect(root).toHaveAttribute('data-witness-missing-command-delta', '0');
		await expect(root).toHaveAttribute('data-witness-missing-write-delta', '0');
		await expect(root).toHaveAttribute('data-witness-insecure-availability', 'disabled');
		await expect(root).toHaveAttribute(
			'data-witness-insecure-action-result',
			'blocked:action-unavailable',
		);
		await expect(root).toHaveAttribute(
			'data-witness-insecure-effect-result',
			'failed:browser-clipboard-insecure-context',
		);
		await expect(root).toHaveAttribute(
			'data-witness-insecure-feedback',
			'This terminal action is unavailable.',
		);
		await expect(root).toHaveAttribute('data-witness-insecure-command-delta', '0');
		await expect(root).toHaveAttribute('data-witness-insecure-write-delta', '0');
		await expect(root).toHaveAttribute(
			'data-witness-writes',
			'LAN Browser selection|LAN Browser selection',
		);
	} finally {
		await page.context().browser()?.close();
	}
});

test('terminal-parity/IN-14-shared-focus', async () => {
	test.setTimeout(45_000);
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-shared-focus-fixture.html', currentDevelopmentRendererUrl).href,
		);
		const root = page.locator('#terminal-shared-focus-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-forced-canvas', 'true');
		await expect(root).toHaveAttribute('data-witness-same-terminal', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-synthetic-event-scope',
			'application-layer-only',
		);
		await expect(root).toHaveAttribute('data-witness-aggregate-transitions', 'in,out');
		await expect(root).toHaveAttribute('data-witness-claims-after-first-release', '1');
		await expect(root).toHaveAttribute('data-witness-claims-after-pause', '0');
		await expect(root).toHaveAttribute('data-witness-activation-replay-delta', '0');
		await expect(root).toHaveAttribute(
			'data-witness-demand-commands',
			'first:active,second:active,second:paused,second:active',
		);
		await expect(root).toHaveAttribute(
			'data-witness-focus-commands',
			'first:true,second:true,first:false',
		);
	} finally {
		await browser.close();
	}
});

test('terminal-parity/SU-10-scrollbar', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');

	const input = page.getByRole('textbox');
	await input.pressSequentially(
		`i=1; while [ "$i" -le 180 ]; do printf 'su10-history-%03d\\n' "$i"; i=$((i+1)); done; printf 'su10-tail-marker\\n'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibility = root.locator('[role="list"]');
	await expect(accessibility).toContainText('su10-tail-marker');
	const accessibleRows = accessibility.locator(':scope > div');
	const canvas = root.locator('canvas').first();
	await expect(canvas).toHaveAttribute('aria-hidden', 'true');
	expect(await canvas.getAttribute('role')).toBeNull();
	const scrollbar = root.locator('[data-projected-terminal-history-scrollbar="true"]');
	const scrollbarThumb = root.locator('[data-projected-terminal-history-scrollbar-thumb="true"]');
	await expect(scrollbar).toBeVisible();
	await expect(scrollbarThumb).toHaveCSS('border-radius', '9999px');
	await page.clock.install();
	await page.clock.pauseAt((await page.evaluate(() => Date.now())) + 1_000);

	const bounds = await root.boundingBox();
	if (bounds === null) throw new Error('Terminal surface has no bounding box.');
	const visibleRowCount = Number(await root.getAttribute('data-projected-terminal-rows'));
	expect(visibleRowCount).toBeGreaterThan(0);
	const firstVisibleRow = async (): Promise<bigint> =>
		BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
	const bottomStart = await firstVisibleRow();
	const scrollbarX = bounds.x + bounds.width - 4;
	const upperTrackY = bounds.y + bounds.height * 0.25;
	const lowerTrackY = bounds.y + bounds.height * 0.75;

	await page.mouse.click(scrollbarX, upperTrackY);
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
	const upperStart = await firstVisibleRow();
	await page.mouse.move(scrollbarX, upperTrackY);
	await page.mouse.down();
	await page.mouse.move(scrollbarX, lowerTrackY, { steps: 4 });
	await expect.poll(firstVisibleRow).toBeGreaterThan(upperStart);
	await page.mouse.up();

	await input.focus();
	await input.press('ArrowRight');
	await expect(accessibility).toContainText('su10-tail-marker');
	await expect.poll(firstVisibleRow).toBe(bottomStart);
	await page.clock.runFor(1_517);
	await expect(scrollbarThumb).toHaveCSS('opacity', '0');
	const faded = await captureTerminalScrollbarStrip(page, root);

	await canvas.hover();
	await page.mouse.wheel(0, -bounds.height / visibleRowCount);
	await page.clock.runFor(17);
	await expect(scrollbarThumb).toHaveCSS('opacity', '0.5');
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
	const scrollbarBounds = await scrollbar.boundingBox();
	if (scrollbarBounds === null) throw new Error('Terminal scrollbar has no bounding box.');
	const terminalBorderRightWidth = await root.evaluate((element) =>
		Number.parseFloat(getComputedStyle(element).borderRightWidth),
	);
	expect(scrollbarBounds.width).toBe(16);
	expect(scrollbarBounds.x + scrollbarBounds.width).toBeCloseTo(
		bounds.x + bounds.width - terminalBorderRightWidth,
		5,
	);
	const visible = await captureTerminalScrollbarStrip(page, root);
	expect(visible.equals(faded)).toBe(false);
	await page.clock.runFor(1_482);
	await expect(scrollbarThumb).toHaveCSS('opacity', '0.5');
	expect((await captureTerminalScrollbarStrip(page, root)).equals(visible)).toBe(true);
	await page.clock.runFor(18);
	await expect(scrollbarThumb).toHaveCSS('opacity', '0');
	expect((await captureTerminalScrollbarStrip(page, root)).equals(faded)).toBe(true);
});

test('terminal-parity/SU-11-smooth-scroll', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	await input.pressSequentially('seq 180', { delay: 5 });
	await input.press('Enter');
	const accessibility = root.locator('[role="list"]');
	await expect
		.poll(async () => Number(await accessibility.getAttribute('data-terminal-retained-rows')))
		.toBeGreaterThan(180);
	const accessibleRows = accessibility.locator(':scope > div');
	const canvas = root.locator('canvas').first();
	const motion = root.locator('[data-projected-terminal-history-motion="true"]');
	const firstVisibleRow = async (): Promise<bigint> =>
		BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
	const canvasOffset = async (): Promise<number> =>
		canvas.evaluate((element) => {
			if (!(element instanceof HTMLCanvasElement)) {
				throw new Error('Projected terminal surface is not a canvas.');
			}
			const match = /^translateY\((-?[\d.]+)px\)$/u.exec(element.style.transform);
			return match === null ? 0 : Number(match[1]);
		});

	await page.clock.install();
	await page.clock.pauseAt((await page.evaluate(() => Date.now())) + 1_000);
	const bounds = await root.boundingBox();
	if (bounds === null) throw new Error('Terminal surface has no bounding box.');
	const visibleRowCount = Number(await root.getAttribute('data-projected-terminal-rows'));
	expect(visibleRowCount).toBeGreaterThan(0);
	const bottomStart = await firstVisibleRow();

	await canvas.hover();
	await page.mouse.wheel(0, -bounds.height / visibleRowCount);
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
	await expect(motion).toBeAttached();
	await page.clock.runFor(17);
	const upwardStart = await canvasOffset();
	expect(upwardStart).toBeLessThan(0);
	expect(Math.abs(upwardStart)).toBeLessThanOrEqual(bounds.height);
	await page.clock.runFor(34);
	const upwardMidpoint = await canvasOffset();
	expect(upwardMidpoint).toBeLessThan(0);
	expect(Math.abs(upwardMidpoint)).toBeLessThan(Math.abs(upwardStart));

	await page.mouse.wheel(0, bounds.height / visibleRowCount);
	await expect.poll(firstVisibleRow).toBe(bottomStart);
	await expect(motion).toBeAttached();
	await page.clock.runFor(17);
	const downwardStart = await canvasOffset();
	expect(downwardStart).toBeGreaterThan(0);
	expect(downwardStart).toBeLessThanOrEqual(bounds.height);

	await page.evaluate(() => {
		Object.defineProperty(document, 'visibilityState', {
			configurable: true,
			value: 'hidden',
		});
		document.dispatchEvent(new Event('visibilitychange'));
	});
	await expect(motion).toHaveCount(0);
	expect(await canvasOffset()).toBe(0);
	await page.clock.runFor(200);
	await expect(motion).toHaveCount(0);
	expect(await canvasOffset()).toBe(0);
	await page.evaluate(() => {
		Reflect.deleteProperty(document, 'visibilityState');
		document.dispatchEvent(new Event('visibilitychange'));
	});
});

test('terminal-parity/LC-06-metadata-bridge', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	const selectedTab = page.locator(
		'[data-slot="workbench-tab-bar"] [role="tab"][aria-selected="true"]',
	);

	await input.pressSequentially(`printf '\\033]0;lc06-osc0-title\\007'; sleep 5`, { delay: 1 });
	await input.press('Enter');
	await expect(selectedTab).toContainText('lc06-osc0-title');
	await input.press('Control+C');
	await input.pressSequentially(`printf '\\033]2;lc06-osc2-title\\007'; sleep 5`, { delay: 1 });
	await input.press('Enter');
	await expect(selectedTab).toContainText('lc06-osc2-title');
});

test('terminal-parity/HS-04-selection-gesture', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	await input.pressSequentially(
		`printf '\\033[2J\\033[Hhs04-double doubleword suffix\\nhs04-triple entire row marker\\n'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibility = root.locator('[role="list"]');
	await expect(accessibility).toContainText('hs04-triple entire row marker');
	await waitForTerminalPresentationStability(root);
	const rows = accessibility.locator(':scope > div');
	const canvas = root.locator('canvas');
	const word = await terminalCellPoint(root, canvas, rows, 'hs04-double doubleword suffix', 15);

	await page.mouse.click(word.x, word.y, { clickCount: 2 });
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe('doubleword');

	const line = await terminalCellPoint(root, canvas, rows, 'hs04-triple entire row marker', 5);
	await page.mouse.click(line.x, line.y, { clickCount: 3 });
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toContain('hs04-triple entire row marker');
});

test('terminal-parity/HS-12-sustained-output-interaction', async ({ browserName }, testInfo) => {
	test.setTimeout(90_000);
	test.skip(browserName !== 'chromium', 'The sustained-output witness requires Chromium.');
	test.skip(process.platform === 'win32', 'The sustained-output witness uses a POSIX shell.');
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	const accessibility = root.locator('[role="list"]');
	const accessibleRows = accessibility.locator(':scope > div');
	const canvas = root.locator('canvas').first();
	const stressCommand =
		`i=1; while [ "$i" -le 220 ]; do printf 'hs12-history-%03d selectableword\\n' "$i"; i=$((i+1)); done; ` +
		`node -e 'let i=0; process.stdin.setRawMode(true); process.stdin.resume(); process.stdin.on("data",(data)=>{if(data[0]===3){process.exit(0)}process.stdout.write("\\r\\nhs12-input-ack-"+data.toString("hex")+"\\r\\n")}); setInterval(()=>{process.stdout.write("\\r\\x1b[2Khs12-status-"+String(i++).padStart(6,"0"))},2)'`;
	await input.pressSequentially(stressCommand, { delay: 1 });
	await input.press('Enter');

	try {
		await expect(accessibility).toContainText('hs12-status-', { timeout: 10_000 });
		const firstVisibleRow = async (): Promise<bigint> =>
			BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
		const revisionBeforeScroll = BigInt(
			(await root.getAttribute('data-projected-terminal-rendered-revision')) ?? '0',
		);
		const isAwayFromBottom = async (): Promise<boolean> => {
			const totalRows = BigInt(
				(await accessibility.getAttribute('data-terminal-retained-rows')) ?? '0',
			);
			const visibleRows = BigInt((await root.getAttribute('data-projected-terminal-rows')) ?? '0');
			const maximumStart = totalRows > visibleRows ? totalRows - visibleRows : 0n;
			return (await firstVisibleRow()) < maximumStart;
		};

		await canvas.hover();
		await page.mouse.wheel(0, -2_000);
		await expect.poll(isAwayFromBottom).toBe(true);
		const fineScrollStart = await firstVisibleRow();
		await accessibility.evaluate((element, initialStart) => {
			const terminalRoot = element.closest<HTMLElement>('[data-projected-terminal-state]');
			if (terminalRoot === null) {
				throw new Error('The sustained-output scroll measurement surface is unavailable.');
			}
			const surface = terminalRoot.querySelector('canvas');
			if (surface === null) {
				throw new Error('The sustained-output scroll measurement canvas is unavailable.');
			}
			let startedAt: number | undefined;
			const recordProgress = (): void => {
				const firstRow = element.querySelector<HTMLElement>(':scope > div');
				const currentStart = firstRow?.dataset['terminalAbsoluteRow'];
				if (
					startedAt === undefined ||
					currentStart === undefined ||
					BigInt(currentStart) >= BigInt(initialStart)
				) {
					return;
				}
				terminalRoot.dataset['hs12FineScrollLatencyMilliseconds'] = String(
					performance.now() - startedAt,
				);
				observer.disconnect();
			};
			const observer = new MutationObserver(recordProgress);
			observer.observe(element, { attributes: true, childList: true, subtree: true });
			surface.addEventListener(
				'wheel',
				() => {
					startedAt ??= performance.now();
					recordProgress();
				},
				{ capture: true, once: true },
			);
		}, fineScrollStart.toString());
		for (let index = 0; index < 12; index++) await page.mouse.wheel(0, -12);
		await expect.poll(firstVisibleRow).toBeLessThan(fineScrollStart);
		await expect
			.poll(() => root.getAttribute('data-hs12-fine-scroll-latency-milliseconds'))
			.not.toBeNull();
		const fineScrollLatencyMilliseconds = Number(
			await root.getAttribute('data-hs12-fine-scroll-latency-milliseconds'),
		);
		expect(fineScrollLatencyMilliseconds).toBeLessThanOrEqual(150);

		await input.evaluate((element) => {
			const terminalRoot = element.closest<HTMLElement>('[data-projected-terminal-state]');
			if (terminalRoot === null) {
				throw new Error('The sustained-output input measurement surface is unavailable.');
			}
			const accessibilitySurface = terminalRoot.querySelector<HTMLElement>('[role="list"]');
			if (accessibilitySurface === null) {
				throw new Error('The sustained-output accessibility surface is unavailable.');
			}
			let startedAt: number | undefined;
			const recordAcknowledgement = (): void => {
				if (
					startedAt === undefined ||
					!accessibilitySurface.textContent?.includes('hs12-input-ack-71')
				) {
					return;
				}
				terminalRoot.dataset['hs12InputLatencyMilliseconds'] = String(
					performance.now() - startedAt,
				);
				observer.disconnect();
			};
			const observer = new MutationObserver(recordAcknowledgement);
			observer.observe(accessibilitySurface, { childList: true, subtree: true });
			element.addEventListener(
				'keydown',
				(event) => {
					if ((event as KeyboardEvent).key !== 'q') return;
					startedAt = performance.now();
					recordAcknowledgement();
				},
				{ capture: true, once: true },
			);
		});
		await input.focus();
		await input.press('q');
		await expect(accessibility).toContainText('hs12-input-ack-71');
		await expect
			.poll(() => root.getAttribute('data-hs12-input-latency-milliseconds'))
			.not.toBeNull();
		const inputLatencyMilliseconds = Number(
			await root.getAttribute('data-hs12-input-latency-milliseconds'),
		);
		expect(inputLatencyMilliseconds).toBeLessThanOrEqual(150);

		await canvas.hover();
		await page.mouse.wheel(0, -2_000);
		await expect.poll(isAwayFromBottom).toBe(true);
		const visibleHistoryRow = accessibleRows.filter({ hasText: 'selectableword' }).first();
		await expect(visibleHistoryRow).toBeVisible();
		const visibleText = (await visibleHistoryRow.textContent())?.trim();
		if (visibleText === undefined || !visibleText.includes('selectableword')) {
			throw new Error('The sustained-output history marker is not visible.');
		}
		const wordStart = await terminalCellPoint(
			root,
			canvas,
			accessibleRows,
			'selectableword',
			0,
			0.25,
		);
		const wordEnd = await terminalCellPoint(
			root,
			canvas,
			accessibleRows,
			'selectableword',
			'selectableword'.length - 1,
			0.75,
		);
		await application.evaluate(({ clipboard }) => clipboard.writeText('hs12 clipboard sentinel'));
		await canvas.evaluate((element) => {
			const surface = element as HTMLCanvasElement;
			surface.addEventListener(
				'pointerdown',
				(event) => {
					surface.dataset['hs12PointerId'] = String(event.pointerId);
				},
				{ once: true },
			);
		});
		await page.mouse.move(wordStart.x, wordStart.y);
		await page.mouse.down();
		await expect.poll(() => canvas.getAttribute('data-hs12-pointer-id')).not.toBeNull();
		await canvas.evaluate((element, endpoint) => {
			const surface = element as HTMLCanvasElement;
			const pointerId = Number(surface.dataset['hs12PointerId']);
			surface.dispatchEvent(
				new PointerEvent('pointermove', {
					bubbles: true,
					button: -1,
					buttons: 0,
					clientX: endpoint.x,
					clientY: endpoint.y,
					composed: true,
					isPrimary: true,
					pointerId,
					pointerType: 'mouse',
				}),
			);
			delete surface.dataset['hs12PointerId'];
		}, wordEnd);
		await page.mouse.move(wordEnd.x, wordEnd.y, { steps: 4 });
		await page.mouse.up();
		await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
		await expect
			.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
			.toContain('selectableword');
		await expect
			.poll(async () =>
				BigInt((await root.getAttribute('data-projected-terminal-rendered-revision')) ?? '0'),
			)
			.toBeGreaterThan(revisionBeforeScroll);
		await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
		const failureCode = await root.getAttribute('data-projected-terminal-failure-code');
		if (failureCode !== null) {
			throw new Error(
				`Sustained-output interaction failed: ${failureCode} operation=${await root.getAttribute('data-projected-terminal-failure-operation')} request=${await root.getAttribute('data-projected-terminal-failure-request-sequence')}`,
			);
		}
		await testInfo.attach('hs12-interaction-metrics.json', {
			body: JSON.stringify({ fineScrollLatencyMilliseconds, inputLatencyMilliseconds }),
			contentType: 'application/json',
		});
	} finally {
		await input.focus();
		await input.press('Control+C');
	}
});

test('terminal-parity/HS-07-selection-export', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	await application.evaluate(({ clipboard }) => clipboard.writeText('hs07 clipboard sentinel'));
	await input.pressSequentially(
		`printf '\\033[2J\\033[Hhs07-prefix \\033[31;3mHS07StyledPlain\\033[0m suffix\\n'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibility = root.locator('[role="list"]');
	const visibleText = 'hs07-prefix HS07StyledPlain suffix';
	await expect(accessibility).toContainText(visibleText);
	await waitForTerminalPresentationStability(root);
	const styledWord = await terminalCellPoint(
		root,
		root.locator('canvas'),
		accessibility.locator(':scope > div'),
		visibleText,
		15,
	);

	await page.mouse.click(styledWord.x, styledWord.y, { clickCount: 2 });
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe('HS07StyledPlain');
});

test('terminal-parity/IN-03-image-paste', async () => {
	test.skip(process.platform === 'win32', 'The exact child-input witness uses a POSIX shell.');
	const application = electronApplication;
	const directory = runtimeDirectory;
	if (application === undefined) throw new Error('Electron application did not start.');
	if (directory === undefined) throw new Error('Desktop runtime directory is unavailable.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = root.getByRole('textbox');
	const readyPath = path.join(directory, 'image-paste-ready');
	const witnessPath = path.join(directory, 'image-paste-witness');
	const script = [
		"const fs = require('node:fs')",
		'process.stdin.setRawMode(true)',
		'process.stdin.resume()',
		`fs.writeFileSync(${JSON.stringify(readyPath)}, 'ready')`,
		`process.stdin.once('data', (data) => { fs.writeFileSync(${JSON.stringify(witnessPath)}, data.toString('hex')); process.exit(0) })`,
	].join(';');
	await input.pressSequentially(`node -e ${quotePosixShellWord(script)}`, { delay: 1 });
	await input.press('Enter');
	await expect
		.poll(async () => {
			try {
				await access(readyPath);
				return true;
			} catch {
				return false;
			}
		})
		.toBe(true);
	await application.evaluate(({ clipboard, nativeImage }) => {
		const image = nativeImage.createFromDataURL(
			'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
		);
		if (image.isEmpty()) throw new Error('Image-paste witness could not create clipboard image.');
		clipboard.writeImage(image);
	});

	await input.focus();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyV' : 'Control+Shift+KeyV');
	await expect.poll(async () => readFile(witnessPath, 'utf8')).toBe('16');
	expect(await root.getAttribute('data-projected-terminal-action-id')).not.toBe(
		'terminal.clipboard.paste',
	);
});

test('terminal-parity/IN-13-selection-copy-paste', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = root.getByRole('textbox');
	const selectionText = 'selectioncopytoken';
	await application.evaluate(({ clipboard }) => clipboard.writeText('stale clipboard sentinel'));
	await input.pressSequentially(selectionText, { delay: 1 });
	const accessibility = root.locator('[role="list"]');
	const accessibilityRows = accessibility.locator(':scope > div');
	await expect(accessibility).toContainText(selectionText);
	await waitForTerminalPresentationStability(root);
	const word = await terminalCellPoint(
		root,
		root.locator('canvas'),
		accessibilityRows,
		selectionText,
		Math.floor(selectionText.length / 2),
	);
	await page.mouse.click(word.x, word.y, { clickCount: 2 });
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
	await expect(input).toBeFocused();
	await expect.poll(async () => terminalDocumentSelectionState(root)).not.toBe('terminal-range');
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe(selectionText);
	await application.evaluate(({ clipboard }) => clipboard.writeText('stale clipboard sentinel'));

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyC' : 'Control+Shift+KeyC');
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyV' : 'Control+Shift+KeyV');
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'none');
	await expect(accessibility).toContainText(`${selectionText}${selectionText}`);
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe(selectionText);
});

test('terminal-parity/IN-18-native-selection-paste', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = root.getByRole('textbox');
	const selectionText = 'nativeselectiontoken';
	await input.pressSequentially(selectionText, { delay: 1 });
	const accessibility = root.locator('[role="list"]');
	const accessibilityRows = accessibility.locator(':scope > div');
	await expect(accessibility).toContainText(selectionText);
	await waitForTerminalPresentationStability(root);
	const word = await terminalCellPoint(
		root,
		root.locator('canvas'),
		accessibilityRows,
		selectionText,
		Math.floor(selectionText.length / 2),
	);
	await page.mouse.click(word.x, word.y, { clickCount: 2 });
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
	await expect(input).toBeFocused();
	await expect.poll(async () => terminalDocumentSelectionState(root)).not.toBe('terminal-range');
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe(selectionText);

	const clipboardText = 'nativepastepayload';
	await application.evaluate(({ clipboard }, text) => clipboard.writeText(text), clipboardText);
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyV' : 'Control+Shift+KeyV');
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'none');
	await expect(accessibility).toContainText(`${selectionText}${clipboardText}`);
});

test('terminal-parity/SU-02-font-metrics', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = page.getByRole('textbox');
	await expect
		.poll(() => input.evaluate((element) => element.style.fontFamily))
		.toContain('Orchbee JetBrains Mono');

	const fontEvidence = await page.evaluate(async () => {
		const symbolText = String.fromCodePoint(0xf120, 0xf121, 0xf07c, 0xf15b);
		const [primaryFaces, italicFaces, symbolFaces] = await Promise.all([
			document.fonts.load('400 32px "Orchbee JetBrains Mono"', 'MW'),
			document.fonts.load('italic 400 32px "Orchbee JetBrains Mono"', 'MW'),
			document.fonts.load('400 32px "Orchbee Symbols Nerd Font"', symbolText),
		]);
		const raster = (text: string): { readonly hash: number; readonly ink: number } => {
			const canvas = document.createElement('canvas');
			canvas.height = 64;
			canvas.width = 256;
			const context = canvas.getContext('2d', { willReadFrequently: true });
			if (context === null) throw new Error('Canvas 2D is unavailable.');
			context.fillStyle = '#fff';
			context.font = '400 32px "Orchbee Symbols Nerd Font"';
			context.textBaseline = 'top';
			context.fillText(text, 0, 0);
			const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
			let hash = 2_166_136_261;
			let ink = 0;
			for (const [index, channel] of pixels.entries()) {
				hash = Math.imul(hash ^ channel, 16_777_619);
				if (index % 4 === 3 && channel > 0) ink += 1;
			}
			return { hash: hash >>> 0, ink };
		};
		return {
			italicFaces: italicFaces.map((face) => ({ family: face.family, status: face.status })),
			licenseCount: document.head.querySelectorAll('link[data-orchbee-terminal-font-license]')
				.length,
			missingRaster: raster(String.fromCodePoint(0x10ffff)),
			primaryFaces: primaryFaces.map((face) => ({ family: face.family, status: face.status })),
			symbolFaces: symbolFaces.map((face) => ({ family: face.family, status: face.status })),
			symbolRaster: raster(symbolText),
		};
	});
	expect(fontEvidence.primaryFaces).toEqual([
		expect.objectContaining({
			family: expect.stringContaining('Orchbee JetBrains Mono'),
			status: 'loaded',
		}),
	]);
	expect(fontEvidence.italicFaces).toEqual([
		expect.objectContaining({
			family: expect.stringContaining('Orchbee JetBrains Mono'),
			status: 'loaded',
		}),
	]);
	expect(fontEvidence.symbolFaces).toEqual([
		expect.objectContaining({
			family: expect.stringContaining('Orchbee Symbols Nerd Font'),
			status: 'loaded',
		}),
	]);
	expect(fontEvidence.licenseCount).toBe(2);
	expect(fontEvidence.symbolRaster.ink).toBeGreaterThan(0);
	expect(fontEvidence.symbolRaster.hash).not.toBe(fontEvidence.missingRaster.hash);

	await input.pressSequentially(
		`printf '\\033[2J\\033[Hnerd: \\357\\204\\240 \\357\\204\\241 \\357\\201\\274 \\357\\205\\233\\n'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText(
		`nerd: ${String.fromCodePoint(0xf120)} ${String.fromCodePoint(0xf121)} ${String.fromCodePoint(0xf07c)} ${String.fromCodePoint(0xf15b)}`,
	);
});

test('terminal-parity/GX-01-direct-kitty-graphics', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');
	const input = page.getByRole('textbox');
	await input.pressSequentially(
		`printf '\\033_Ga=T,t=d,f=24,i=71,p=1,s=1,v=1,c=2,r=1,q=2;/wAA\\033\\\\'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const overlay = root.locator('canvas[data-terminal-graphics-overlay="true"]');
	await expect(overlay).toBeAttached();
	await expect(overlay).toHaveAttribute('aria-hidden', 'true');
	await expect(overlay).toHaveCSS('pointer-events', 'none');
	const coloredPixels = async (): Promise<{ readonly green: number; readonly red: number }> =>
		overlay.evaluate((canvas) => {
			if (!(canvas instanceof HTMLCanvasElement))
				throw new Error('Graphics overlay is not a canvas.');
			const context = canvas.getContext('2d', { willReadFrequently: true });
			if (context === null) throw new Error('Graphics overlay has no 2D context.');
			const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
			let green = 0;
			let red = 0;
			for (let index = 0; index < pixels.length; index += 4) {
				if ((pixels[index] ?? 0) > 200 && (pixels[index + 1] ?? 0) < 40) red += 1;
				if ((pixels[index + 1] ?? 0) > 200 && (pixels[index] ?? 0) < 40) green += 1;
			}
			return { green, red };
		});
	await expect.poll(async () => (await coloredPixels()).red).toBeGreaterThan(0);

	await input.pressSequentially(`printf '\\033_Ga=t,t=d,f=24,i=71,s=1,v=1,q=2;AP8A\\033\\\\'`, {
		delay: 1,
	});
	await input.press('Enter');
	await expect.poll(async () => (await coloredPixels()).green).toBeGreaterThan(0);
	await input.pressSequentially(`printf '\\033_Ga=d,d=A,q=2;\\033\\\\'`, { delay: 1 });
	await input.press('Enter');
	await expect(overlay).not.toBeAttached();
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test('terminal-parity/FB-01-clipboard-bell-progress', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');
	const input = page.getByRole('textbox');
	const requestedText = 'fb01 exact terminal clipboard text';
	const encodedText = Buffer.from(requestedText, 'utf8').toString('base64');
	await application.evaluate(({ clipboard }) =>
		clipboard.writeText('fb01 existing clipboard sentinel'),
	);

	const sendClipboardRequest = async (): Promise<void> => {
		await input.focus();
		await input.pressSequentially(`printf '\\033]52;c;${encodedText}\\033\\\\'`, {
			delay: 1,
		});
		await input.press('Enter');
	};
	await sendClipboardRequest();
	await expect(page.locator('[data-slot="projected-terminal-clipboard-review"]')).toHaveCount(0);
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toBe(requestedText);

	await input.focus();
	await input.pressSequentially(`for i in 1 2 3 4 5; do printf '\\007'; sleep 0.1; done`, {
		delay: 1,
	});
	await input.press('Enter');
	await expect(root).toHaveAttribute('data-projected-terminal-bell-active', 'true');
	await expect(root).not.toHaveAttribute('data-projected-terminal-bell-active', 'true');

	await input.pressSequentially(`printf '\\033]9;4;1;42\\033\\\\'`, { delay: 1 });
	await input.press('Enter');
	const progress = root.locator('[data-projected-terminal-progress="true"]');
	await expect(progress).toBeVisible();
	await expect(progress).toHaveAttribute('data-state', 'set');
	await expect(progress).toHaveAttribute('aria-valuenow', '42');
	await input.pressSequentially(`printf '\\033]9;4;0;\\033\\\\'`, { delay: 1 });
	await input.press('Enter');
	await expect(progress).toBeHidden();
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test('terminal-parity/SI-01-manual-shell-integration-completion-reminder', async () => {
	test.skip(process.platform !== 'darwin', 'The current real Zsh witness is verified on macOS.');
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');

	const settingsSurface = await openDesktopSettingsCategory(page, 'shell-integration');
	await expect(settingsSurface).toContainText(
		/never edits shell startup files|永远不会编辑 Shell 启动文件/u,
	);
	const zshCommand = await settingsSurface
		.locator('code')
		.filter({ hasText: 'orchbee.zsh' })
		.innerText();
	const reminderMode = settingsSurface.locator('[data-slot="completion-reminder-mode"]');
	await reminderMode.selectOption('always');
	await expect(reminderMode).toHaveValue('always');
	const minimumDuration = settingsSurface.locator('[data-slot="completion-reminder-minimum"]');
	await minimumDuration.fill('1');
	await minimumDuration.press('Enter');
	await expect(minimumDuration).toHaveValue('1');
	await settingsSurface.locator('aside > button').first().click();
	await expect(settingsSurface).not.toBeVisible();
	expect(await page.evaluate(() => localStorage.getItem('orchbee:web:terminal-feedback:v1'))).toBe(
		'{"completionReminderMinimumSeconds":1,"completionReminderMode":"always"}',
	);

	const input = page.getByRole('textbox');
	await input.pressSequentially('exec /bin/zsh -f', { delay: 1 });
	await input.press('Enter');
	await page.waitForTimeout(250);
	await input.pressSequentially(zshCommand, { delay: 1 });
	await input.press('Enter');
	await page.waitForTimeout(250);
	await input.pressSequentially('sleep 1.1', { delay: 1 });
	await input.press('Enter');

	await expect(root).toHaveAttribute(
		'data-projected-terminal-command-completion-observed-sequence',
		/\d+/u,
	);
	await expect(root).toHaveAttribute(
		'data-projected-terminal-command-reminder-decision',
		/^remind-(?:shown|unavailable)$/u,
	);
	await expect(root).toHaveAttribute('data-projected-terminal-command-completion-sequence', /\d+/u);
	await expect(root).toHaveAttribute(
		'data-projected-terminal-command-notification',
		/^(?:shown|unavailable)$/u,
	);
	await expect(root.locator('[data-projected-terminal-status="true"]')).toContainText(
		/completed|完成/u,
	);
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test('terminal-parity/SI-02-prompt-navigation', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');
	const input = page.getByRole('textbox');
	const accessibility = root.locator('[role="list"]');
	const accessibleRows = accessibility.locator(':scope > div');
	const firstVisibleRow = async (): Promise<bigint> =>
		BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
	const pressPromptShortcut = async (key: 'KeyX' | 'KeyZ'): Promise<void> => {
		await root.evaluate((element) => {
			delete (element as HTMLElement).dataset['projectedTerminalActionId'];
			delete (element as HTMLElement).dataset['projectedTerminalActionResult'];
		});
		await page.keyboard.press(`Control+Shift+${key}`);
		await expect(root).toHaveAttribute(
			'data-projected-terminal-action-id',
			key === 'KeyZ' ? 'terminal.history.prompt-previous' : 'terminal.history.prompt-next',
		);
		await expect(root).toHaveAttribute('data-projected-terminal-action-result', 'completed');
	};

	await input.pressSequentially(
		[
			"printf '\\033]133;A\\007si02-prompt-one\\033]133;B\\007\\n\\033]133;C\\007'",
			'i=1; while [ "$i" -le 36 ]; do printf \'si02-one-%03d\\n\' "$i"; i=$((i+1)); done',
			"printf '\\033]133;D;0\\007\\033]133;A\\007si02-prompt-two\\033]133;B\\007\\n\\033]133;C\\007'",
			'i=1; while [ "$i" -le 36 ]; do printf \'si02-two-%03d\\n\' "$i"; i=$((i+1)); done',
			"printf '\\033]133;D;0\\007\\033]133;A\\007si02-prompt-three\\033]133;B\\007\\n\\033]133;C\\007'",
			'i=1; while [ "$i" -le 48 ]; do printf \'si02-tail-%03d\\n\' "$i"; i=$((i+1)); done',
			"printf '\\033]133;D;0\\007\\033]133;B\\007'",
			'exec sleep 60',
		].join('; '),
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(accessibility).toContainText('si02-tail-048');
	await expect(root).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	await page.waitForTimeout(1_000);
	await waitForTerminalPresentationStability(root);
	await expect(input).toBeFocused();
	const bottomStart = await firstVisibleRow();

	await pressPromptShortcut('KeyZ');
	await expect(accessibleRows.first()).toContainText('si02-prompt-three');
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
	const thirdPromptStart = await firstVisibleRow();
	await expect(input).toBeFocused();

	await pressPromptShortcut('KeyZ');
	await expect(accessibleRows.first()).toContainText('si02-prompt-two');
	await expect.poll(firstVisibleRow).toBeLessThan(thirdPromptStart);
	await expect(input).toBeFocused();

	await pressPromptShortcut('KeyX');
	await expect(accessibleRows.first()).toContainText('si02-prompt-three');
	await expect.poll(firstVisibleRow).toBe(thirdPromptStart);
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test(automaticShellIntegrationWitnessTitle, async () => {
	test.skip(
		process.platform !== 'darwin',
		'The current automatic Zsh witness is verified on macOS.',
	);
	const application = electronApplication;
	const currentZdotdir = runtimeDirectory && path.join(runtimeDirectory, 'zsh-config');
	if (application === undefined || currentZdotdir === undefined) {
		throw new Error('Desktop automatic shell-integration witness is unavailable.');
	}
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');

	const settingsSurface = await openDesktopSettingsCategory(page, 'shell-integration');
	await expect(settingsSurface).toContainText(/automatically loads integration|自动加载集成/u);
	await expect(settingsSurface).toContainText(
		/manual fallback source command|手动回退 source 命令/u,
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();

	const input = page.getByRole('textbox');
	await input.pressSequentially(
		'printf \'si03-state=%s:%s:%s:%s:%s\\n\' "$ORCHBEE_SI_ZSHENV_WITNESS" "$ORCHBEE_SI_ZPROFILE_WITNESS" "$ORCHBEE_SI_ZSHRC_WITNESS" "$ZDOTDIR" "${ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR-unset}"',
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText(
		`si03-state=preserved:preserved:preserved:${currentZdotdir}:unset`,
	);
	await expect(root).toHaveAttribute(
		'data-projected-terminal-command-completion-observed-sequence',
		/\d+/u,
	);
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test('terminal-parity/HS-11-keyboard-scrollback', async () => {
	test.setTimeout(45_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await root.getAttribute('data-projected-terminal-attachment-id');
	const input = page.getByRole('textbox');
	const accessibility = root.locator('[role="list"]');
	const accessibleRows = accessibility.locator(':scope > div');
	const firstVisibleRow = async (): Promise<bigint> =>
		BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
	const pressHistoryShortcut = async (shortcut: string, actionId: string): Promise<void> => {
		await root.evaluate((element) => {
			delete (element as HTMLElement).dataset['projectedTerminalActionId'];
			delete (element as HTMLElement).dataset['projectedTerminalActionResult'];
		});
		await page.keyboard.press(shortcut);
		await expect(root).toHaveAttribute('data-projected-terminal-action-id', actionId);
		await expect(root).toHaveAttribute('data-projected-terminal-action-result', 'completed');
	};

	await input.pressSequentially(
		`i=1; while [ "$i" -le 180 ]; do printf 'in15-history-%03d\\n' "$i"; i=$((i+1)); done; printf 'in15-tail-marker\\n'; exec sleep 60`,
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(accessibility).toContainText('in15-tail-marker');
	await waitForTerminalPresentationStability(root);
	await expect(input).toBeFocused();
	const bottomStart = await firstVisibleRow();

	await pressHistoryShortcut('Control+Shift+PageUp', 'terminal.history.scroll-page-up');
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
	const pageStart = await firstVisibleRow();

	await pressHistoryShortcut('Control+Shift+ArrowUp', 'terminal.history.scroll-line-up');
	await expect.poll(firstVisibleRow).toBeLessThan(pageStart);

	await pressHistoryShortcut('Control+Shift+ArrowDown', 'terminal.history.scroll-line-down');
	await expect.poll(firstVisibleRow).toBe(pageStart);

	await pressHistoryShortcut('Control+Shift+Home', 'terminal.history.scroll-top');
	await expect.poll(firstVisibleRow).toBe(0n);

	await pressHistoryShortcut('Control+Shift+PageDown', 'terminal.history.scroll-page-down');
	await expect.poll(firstVisibleRow).toBeGreaterThan(0n);

	await pressHistoryShortcut('Control+Shift+End', 'terminal.history.scroll-bottom');
	await expect.poll(firstVisibleRow).toBe(bottomStart);
	await expect(input).toBeFocused();
	expect(await root.getAttribute('data-projected-terminal-attachment-id')).toBe(attachmentId);
});

test('projects native window drag regions without consuming title bar controls or shortcuts', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const desktopWorkbench = page.locator('[data-slot="desktop-workbench"]');
	await expect(desktopWorkbench).toHaveAttribute(
		'data-window-controls',
		process.platform === 'darwin' ? 'leading' : 'trailing',
	);

	const appRegion = (locator: Locator): Promise<string> =>
		locator.evaluate((element) => {
			const style = getComputedStyle(element);
			return style.getPropertyValue('app-region') || style.getPropertyValue('-webkit-app-region');
		});
	const leftChrome = page.locator('[data-slot="workbench-window-drag-region"]');
	const sidebar = page.locator('[data-slot="workbench-sidebar"]');
	const sidebarTogglePosition = page.locator('[data-slot="workbench-sidebar-toggle-position"]');
	const sidebarToggle = page.locator('[data-slot="workbench-sidebar-toggle"]');
	const tabTrack = page.locator(
		'[data-slot="workbench-tab-bar"][data-orientation="horizontal"] [data-horizontal-track]',
	);
	const selectedTab = tabTrack.locator('[role="tab"][aria-selected="true"]');
	const newTabButton = tabTrack.locator(':scope > button').last();

	await expect.poll(() => appRegion(leftChrome)).toBe('drag');
	await expect.poll(() => appRegion(tabTrack)).toBe('drag');
	await expect.poll(() => appRegion(sidebarTogglePosition)).toBe('no-drag');
	await expect.poll(() => appRegion(sidebarToggle)).toBe('no-drag');
	await expect.poll(() => appRegion(selectedTab)).toBe('no-drag');
	await expect.poll(() => appRegion(newTabButton)).toBe('no-drag');
	await expect(sidebar).toHaveCSS('padding-top', '36px');

	const [leftChromeBox, sidebarTogglePositionBox, sidebarToggleBox, tabTrackBox] =
		await Promise.all([
			leftChrome.boundingBox(),
			sidebarTogglePosition.boundingBox(),
			sidebarToggle.boundingBox(),
			tabTrack.boundingBox(),
		]);
	if (!leftChromeBox || !sidebarTogglePositionBox || !sidebarToggleBox || !tabTrackBox) {
		throw new Error('Expected complete native window chrome geometry.');
	}
	expect(leftChromeBox.height).toBe(36);
	expect(tabTrackBox.height).toBe(36);
	expect(sidebarTogglePositionBox.y - tabTrackBox.y).toBe(4);
	expect(sidebarToggleBox.height).toBe(28);
	expect(sidebarToggleBox.y + sidebarToggleBox.height / 2).toBe(
		tabTrackBox.y + tabTrackBox.height / 2,
	);

	const terminalInput = page.getByRole('textbox');
	const leftPanel = page.locator('[data-slot="workbench-left-panel"]');
	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await expect(leftPanel).toBeVisible();
	await terminalInput.focus();
	await page.keyboard.press(
		process.platform === 'darwin' ? `${primaryModifier}+KeyB` : `${primaryModifier}+Shift+KeyB`,
	);
	await expect(leftPanel).not.toBeAttached();
	await expect(terminalInput).toBeFocused();
	await sidebarToggle.click();
	await expect(leftPanel).toBeVisible();

	const tabs = workbenchTabs(page);
	const initialTabCount = await tabs.count();
	await newTabButton.click();
	await expect(tabs).toHaveCount(initialTabCount + 1);
	await tabs.first().click();
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press(`${primaryModifier}+Digit2`);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press(`${primaryModifier}+Digit1`);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	const nextTabShortcut =
		process.platform === 'darwin'
			? `${primaryModifier}+Shift+BracketRight`
			: `${primaryModifier}+PageDown`;
	const previousTabShortcut =
		process.platform === 'darwin'
			? `${primaryModifier}+Shift+BracketLeft`
			: `${primaryModifier}+PageUp`;
	await page.keyboard.press(nextTabShortcut);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press(nextTabShortcut);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press(previousTabShortcut);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press(previousTabShortcut);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
});

test('terminal-parity/FS-10-active-tab-movement', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const tabs = workbenchTabs(page);
	const newTabButton = page.getByRole('button', { name: /^(?:New tab|新建标签页)$/u });
	const tabOrder = async (): Promise<readonly string[]> =>
		await tabs.evaluateAll((elements) =>
			elements.map((element) => element.getAttribute('data-tab-id') ?? ''),
		);

	await expect(tabs).toHaveCount(1);
	const firstTabId = await tabs.first().getAttribute('data-tab-id');
	if (firstTabId === null) throw new Error('The initial Tab has no stable identity.');
	await newTabButton.click();
	await expect(tabs).toHaveCount(2);
	const secondTabId = await tabs.nth(1).getAttribute('data-tab-id');
	if (secondTabId === null) throw new Error('The second Tab has no stable identity.');

	await tabs.first().click();
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	const activeRoot = projectedTerminal(page);
	const presentationId = await activeRoot.getAttribute('data-terminal-presentation-id');
	if (presentationId === null) throw new Error('The active terminal has no presentation identity.');
	const activeInput = activeRoot.getByRole('textbox');
	await activeInput.focus();

	await page.keyboard.press('Control+Shift+Period');
	await expect.poll(tabOrder).toEqual([secondTabId, firstTabId]);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	await expect(activeInput).toBeFocused();

	await page.keyboard.press('Control+Shift+Period');
	await expect.poll(tabOrder).toEqual([firstTabId, secondTabId]);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');

	await page.keyboard.press('Control+Shift+Comma');
	await expect.poll(tabOrder).toEqual([secondTabId, firstTabId]);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '向前移动活动标签页'
			: 'Move active tab forward',
	);
	const moveForwardOption = searchSurface.getByRole('option', {
		name: /(?:Move active tab forward|向前移动活动标签页)/u,
	});
	await expect(moveForwardOption).toHaveAttribute('aria-disabled', 'false');
	await moveForwardOption.click();
	await expect(searchSurface).not.toBeAttached();
	await expect.poll(tabOrder).toEqual([firstTabId, secondTabId]);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');

	const recoveredRoot = projectedTerminalFor(page, presentationId);
	const recoveredInput = recoveredRoot.getByRole('textbox');
	await recoveredInput.focus();
	await recoveredInput.pressSequentially("printf 'active-tab-movement-alive\\n'", { delay: 1 });
	await recoveredInput.press('Enter');
	await expect(recoveredRoot.locator('[role="list"]')).toContainText('active-tab-movement-alive');
});

test('terminal-parity/FS-11-tab-title', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	const input = root.getByRole('textbox');
	const selectedTab = page.locator(
		'[data-slot="workbench-tab-bar"] [role="tab"][aria-selected="true"]',
	);
	const titleShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+KeyI' : 'Control+Alt+Shift+KeyT';

	await input.focus();
	await input.pressSequentially(
		"printf '\\033]0;fs11-natural-1\\007'; printf 'fs11-title-ready-1\\n'; sleep 30",
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText('fs11-title-ready-1');
	await expect(selectedTab).toContainText('fs11-natural-1');

	await page.keyboard.press(titleShortcut);
	let dialog = page.getByRole('dialog', { name: /^(?:Set Tab Title|设置标签页标题)$/u });
	await expect(dialog).toBeVisible();
	let titleInput = dialog.getByRole('textbox', { name: /^(?:Tab title|标签页标题)$/u });
	await expect(titleInput).toHaveValue('fs11-natural-1');
	await titleInput.fill('FS11 custom title');
	await dialog.getByRole('button', { name: /^(?:Save|保存)$/u }).click();
	await expect(dialog).not.toBeAttached();
	await expect(selectedTab).toContainText('FS11 custom title');
	await expect(input).toBeFocused();

	await input.press('Control+C');
	await input.pressSequentially(
		"printf '\\033]0;fs11-natural-2\\007'; printf 'fs11-title-ready-2\\n'; sleep 30",
		{ delay: 1 },
	);
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText('fs11-title-ready-2');
	await expect(selectedTab).toContainText('FS11 custom title');

	await selectedTab.click({ button: 'right' });
	const tabMenu = page.getByRole('menu', { name: /^(?:Tab options|标签页选项)$/u });
	await expect(tabMenu).toBeVisible();
	await tabMenu.getByRole('menuitem', { name: /^(?:Set Tab Title…|设置标签页标题…)/u }).click();
	dialog = page.getByRole('dialog', { name: /^(?:Set Tab Title|设置标签页标题)$/u });
	await expect(dialog).toBeVisible();
	titleInput = dialog.getByRole('textbox', { name: /^(?:Tab title|标签页标题)$/u });
	await expect(titleInput).toHaveValue('FS11 custom title');
	await titleInput.fill('');
	await dialog.getByRole('button', { name: /^(?:Save|保存)$/u }).click();
	await expect(dialog).not.toBeAttached();
	await expect(selectedTab).toContainText('fs11-natural-2');
	await expect(input).toBeFocused();

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '设置标签页标题'
			: 'Set Tab Title',
	);
	await searchSurface.getByRole('option', { name: /(?:Set Tab Title|设置标签页标题)/u }).click();
	dialog = page.getByRole('dialog', { name: /^(?:Set Tab Title|设置标签页标题)$/u });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole('textbox', { name: /^(?:Tab title|标签页标题)$/u })).toHaveValue(
		'fs11-natural-2',
	);
	await dialog.getByRole('button', { name: /^(?:Cancel|取消)$/u }).click();
	await expect(dialog).not.toBeAttached();
	await expect(input).toBeFocused();
	await input.press('Control+C');
});

test('terminal-parity/FS-12-panel-resize', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const splitBelowShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+KeyD' : 'Control+Shift+KeyE';
	const resizeShortcut = process.platform === 'darwin' ? 'Meta+KeyR' : 'Control+Shift+KeyR';
	const activeLayer = page.locator('[data-presentation-mode="active"]');
	const panels = activeLayer.locator('[data-slot="workbench-panel"]');

	await projectedTerminal(page).getByRole('textbox').focus();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(2);
	await page.keyboard.press(splitBelowShortcut);
	await expect(panels).toHaveCount(3);

	const activePanel = activeLayer.locator('[data-slot="workbench-panel"][data-focused="true"]');
	const activePanelId = await activePanel.getAttribute('data-panel-id');
	if (activePanelId === null) throw new Error('The resize target Panel has no stable identity.');
	const activeInput = activePanel.getByRole('textbox');
	const presentationIds = await activeLayer
		.locator('[data-slot="workbench-terminal-viewport"]')
		.evaluateAll((elements) =>
			elements.map((element) => element.getAttribute('data-terminal-presentation-id') ?? ''),
		);
	const attachmentByPresentation = new Map<string, string | null>();
	const identityByPresentation = new Map<
		string,
		Awaited<ReturnType<typeof terminalDocumentIdentity>>
	>();
	for (const presentationId of presentationIds) {
		const terminalRoot = projectedTerminalFor(page, presentationId);
		attachmentByPresentation.set(
			presentationId,
			await terminalRoot.getAttribute('data-projected-terminal-attachment-id'),
		);
		identityByPresentation.set(presentationId, await terminalDocumentIdentity(terminalRoot));
	}
	const panelBox = async () => {
		const box = await activePanel.boundingBox();
		if (box === null) throw new Error('The active Panel has no layout box.');
		return box;
	};
	const initialBox = await panelBox();

	await activeInput.focus();
	await page.keyboard.press(resizeShortcut);
	let dialog = page.getByRole('dialog', { name: /^(?:Resize Panel|调整面板大小)$/u });
	await expect(dialog).toBeVisible();
	await page.keyboard.press('KeyW');
	await expect.poll(async () => (await panelBox()).width).toBeGreaterThan(initialBox.width + 10);
	const normalWidth = (await panelBox()).width;
	await page.keyboard.press('Control+KeyW');
	await expect.poll(async () => (await panelBox()).width).toBeGreaterThan(normalWidth + 20);
	await page.keyboard.press('KeyT');
	await expect.poll(async () => (await panelBox()).height).toBeGreaterThan(initialBox.height + 10);
	await page.keyboard.press('KeyR');
	await expect
		.poll(async () => {
			const box = await panelBox();
			return { height: Math.round(box.height), width: Math.round(box.width) };
		})
		.toEqual({ height: Math.round(initialBox.height), width: Math.round(initialBox.width) });
	await dialog.getByRole('button', { name: /^(?:Done|完成)/u }).click();
	await expect(dialog).not.toBeAttached();
	await expect(activeInput).toBeFocused();
	for (const presentationId of presentationIds) {
		expect(
			await projectedTerminalFor(page, presentationId).getAttribute(
				'data-projected-terminal-attachment-id',
			),
		).toBe(attachmentByPresentation.get(presentationId));
	}

	const activeViewport = activePanel.locator('[data-slot="workbench-terminal-viewport"]');
	await activeViewport.click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^(?:Resize Panel…|调整面板大小…)/u }).click();
	dialog = page.getByRole('dialog', { name: /^(?:Resize Panel|调整面板大小)$/u });
	await expect(dialog).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(dialog).not.toBeAttached();
	await expect(activeInput).toBeFocused();

	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '调整面板大小'
			: 'Resize Panel',
	);
	await searchSurface.getByRole('option', { name: /(?:Resize Panel|调整面板大小)/u }).click();
	dialog = page.getByRole('dialog', { name: /^(?:Resize Panel|调整面板大小)$/u });
	await expect(dialog).toBeVisible();

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyT' : 'Control+Shift+KeyT');
	await expect(workbenchTabs(page)).toHaveCount(2);
	await expect(dialog).not.toBeAttached();
	await workbenchTabs(page).first().click();
	await expect(panels).toHaveCount(3);
	for (const presentationId of presentationIds) {
		const originalIdentity = identityByPresentation.get(presentationId);
		if (originalIdentity === undefined) throw new Error('Missing terminal identity witness.');
		expect(
			await terminalDocumentIdentity(projectedTerminalFor(page, presentationId)),
		).toMatchObject({
			creationId: originalIdentity.creationId,
			terminalId: originalIdentity.terminalId,
		});
	}
});

test('terminal-parity/FS-13-directional-panel-movement', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const splitBelowShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+KeyD' : 'Control+Shift+KeyE';
	const moveModifier = process.platform === 'darwin' ? 'Meta+Alt+Shift' : 'Control+Alt+Shift';
	const activeLayer = page.locator('[data-presentation-mode="active"]');
	const panels = activeLayer.locator('[data-slot="workbench-panel"]');
	const focusedPanel = activeLayer.locator('[data-slot="workbench-panel"][data-focused="true"]');
	const focusedPanelId = async (): Promise<string> => {
		const panelId = await focusedPanel.getAttribute('data-panel-id');
		if (panelId === null) throw new Error('The focused Panel has no stable identity.');
		return panelId;
	};
	const panelFor = (panelId: string): Locator =>
		activeLayer.locator(`[data-slot="workbench-panel"][data-panel-id="${panelId}"]`);
	const panelRect = async (panelId: string) => {
		const box = await panelFor(panelId).boundingBox();
		if (box === null) throw new Error(`Panel ${panelId} has no layout box.`);
		return {
			height: Math.round(box.height),
			width: Math.round(box.width),
			x: Math.round(box.x),
			y: Math.round(box.y),
		};
	};
	const expectPanelAt = async (panelId: string, expectedPanelId: string): Promise<void> => {
		const expected = initialRects.get(expectedPanelId);
		if (expected === undefined) throw new Error('Missing an initial Panel geometry witness.');
		await expect.poll(() => panelRect(panelId)).toEqual(expected);
	};
	const recordNextKeyDefault = async (code: string): Promise<void> => {
		await page.evaluate((expectedCode) => {
			document.documentElement.removeAttribute('data-fs13-key-default-prevented');
			const record = (event: KeyboardEvent): void => {
				if (event.code !== expectedCode) return;
				document.documentElement.dataset['fs13KeyDefaultPrevented'] = String(
					event.defaultPrevented,
				);
				document.removeEventListener('keydown', record, true);
			};
			document.addEventListener('keydown', record, true);
		}, code);
	};

	await projectedTerminal(page).getByRole('textbox').focus();
	const firstPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(2);
	const secondPanelId = await focusedPanelId();
	await page.keyboard.press(splitBelowShortcut);
	await expect(panels).toHaveCount(3);
	const thirdPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(4);
	const movedPanelId = await focusedPanelId();
	const panelIds = [firstPanelId, secondPanelId, thirdPanelId, movedPanelId];
	expect(new Set(panelIds).size).toBe(4);

	const initialRects = new Map<string, Awaited<ReturnType<typeof panelRect>>>();
	const presentationByPanel = new Map<string, string>();
	const identityByPresentation = new Map<
		string,
		Awaited<ReturnType<typeof terminalDocumentIdentity>>
	>();
	for (const panelId of panelIds) {
		initialRects.set(panelId, await panelRect(panelId));
		const presentationId = await panelFor(panelId)
			.locator('[data-slot="workbench-terminal-viewport"]')
			.getAttribute('data-terminal-presentation-id');
		if (presentationId === null) throw new Error('A Panel has no terminal presentation identity.');
		presentationByPanel.set(panelId, presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		identityByPresentation.set(presentationId, await terminalDocumentIdentity(terminalRoot));
	}

	await recordNextKeyDefault('ArrowLeft');
	await page.keyboard.press(`${moveModifier}+ArrowLeft`);
	await expectPanelAt(movedPanelId, thirdPanelId);
	await expectPanelAt(thirdPanelId, movedPanelId);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', movedPanelId);
	await expect(panelFor(movedPanelId).getByRole('textbox')).toBeFocused();
	await expect(page.locator('html')).toHaveAttribute('data-fs13-key-default-prevented', 'true');

	await page.keyboard.press(`${moveModifier}+ArrowUp`);
	await expectPanelAt(movedPanelId, secondPanelId);
	await expectPanelAt(secondPanelId, thirdPanelId);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', movedPanelId);
	await expect(panelFor(movedPanelId).getByRole('textbox')).toBeFocused();

	await page.keyboard.press(`${moveModifier}+ArrowLeft`);
	await expectPanelAt(movedPanelId, firstPanelId);
	await expectPanelAt(firstPanelId, secondPanelId);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', movedPanelId);
	await expect(panelFor(movedPanelId).getByRole('textbox')).toBeFocused();

	await recordNextKeyDefault('ArrowLeft');
	const edgeRects = new Map(
		await Promise.all(
			panelIds.map(async (panelId) => [panelId, await panelRect(panelId)] as const),
		),
	);
	await page.keyboard.press(`${moveModifier}+ArrowLeft`);
	await expect(page.locator('html')).toHaveAttribute('data-fs13-key-default-prevented', 'false');
	for (const panelId of panelIds) {
		expect(await panelRect(panelId)).toEqual(edgeRects.get(panelId));
	}
	await expect(focusedPanel).toHaveAttribute('data-panel-id', movedPanelId);

	await page.keyboard.press(`${moveModifier}+ArrowRight`);
	await expectPanelAt(movedPanelId, secondPanelId);
	await page.keyboard.press(`${moveModifier}+ArrowDown`);
	await expectPanelAt(movedPanelId, thirdPanelId);
	await page.keyboard.press(`${moveModifier}+ArrowUp`);
	await expectPanelAt(movedPanelId, secondPanelId);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', movedPanelId);
	await expect(panelFor(movedPanelId).getByRole('textbox')).toBeFocused();

	for (const [panelId, presentationId] of presentationByPanel) {
		await expect(
			panelFor(panelId).locator('[data-slot="workbench-terminal-viewport"]'),
		).toHaveAttribute('data-terminal-presentation-id', presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		await expect(terminalRoot).toHaveAttribute(
			'data-projected-terminal-rendered-revision',
			/^\d+$/u,
		);
		const originalIdentity = identityByPresentation.get(presentationId);
		if (originalIdentity === undefined) throw new Error('Missing terminal identity witness.');
		expect(await terminalDocumentIdentity(terminalRoot)).toMatchObject({
			creationId: originalIdentity.creationId,
			terminalId: originalIdentity.terminalId,
		});
	}

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '向左移动面板'
			: 'Move Panel Left',
	);
	await expect(
		searchSurface.getByRole('option', { name: /(?:Move Panel Left|向左移动面板)/u }),
	).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(searchSurface).not.toBeAttached();

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const movementShortcutRow = settingsSurface.locator(
		'[data-shortcut-id="workbench.panel.active.move-left"]',
	);
	await expect(movementShortcutRow).toContainText(/(?:Move active panel left|向左移动活动面板)/u);
	await expect(movementShortcutRow).toContainText(
		process.platform === 'darwin' ? '⌥⇧⌘←' : 'Ctrl+Alt+Shift+←',
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
});

test('terminal-parity/FS-14-panel-equalization', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const equalizeShortcut =
		process.platform === 'darwin' ? 'Control+Meta+Equal' : 'Control+Alt+Equal';
	const maximizeShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+Enter' : 'Control+Shift+Enter';
	const activeLayer = page.locator('[data-presentation-mode="active"]');
	const panels = activeLayer.locator('[data-slot="workbench-panel"]');
	const focusedPanel = activeLayer.locator('[data-slot="workbench-panel"][data-focused="true"]');
	const focusedPanelId = async (): Promise<string> => {
		const panelId = await focusedPanel.getAttribute('data-panel-id');
		if (panelId === null) throw new Error('The focused Panel has no stable identity.');
		return panelId;
	};
	const panelFor = (panelId: string): Locator =>
		activeLayer.locator(`[data-slot="workbench-panel"][data-panel-id="${panelId}"]`);
	const panelRect = async (panelId: string) => {
		const box = await panelFor(panelId).boundingBox();
		if (box === null) throw new Error(`Panel ${panelId} has no layout box.`);
		return {
			height: Math.round(box.height),
			width: Math.round(box.width),
			x: Math.round(box.x),
			y: Math.round(box.y),
		};
	};
	const recordNextEqualizeDefault = async (): Promise<void> => {
		await page.evaluate(() => {
			document.documentElement.removeAttribute('data-fs14-key-default-prevented');
			const record = (event: KeyboardEvent): void => {
				if (event.code !== 'Equal') return;
				document.documentElement.dataset['fs14KeyDefaultPrevented'] = String(
					event.defaultPrevented,
				);
				document.removeEventListener('keydown', record, true);
			};
			document.addEventListener('keydown', record, true);
		});
	};

	const singletonInput = projectedTerminal(page).getByRole('textbox');
	await singletonInput.focus();
	await recordNextEqualizeDefault();
	await page.keyboard.press(equalizeShortcut);
	await expect(page.locator('html')).toHaveAttribute('data-fs14-key-default-prevented', 'false');

	const firstPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(2);
	const secondPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(3);
	const thirdPanelId = await focusedPanelId();
	const panelIds = [firstPanelId, secondPanelId, thirdPanelId];
	expect(new Set(panelIds).size).toBe(3);
	await expect
		.poll(async () => {
			const [first, second, third] = await Promise.all(panelIds.map(panelRect));
			if (first === undefined || second === undefined || third === undefined) {
				throw new Error('Missing initial Panel geometry.');
			}
			return {
				firstIsDouble: first.width > second.width * 1.7,
				nestedAreEqual: Math.abs(second.width - third.width) <= 4,
				sameRow: first.y === second.y && second.y === third.y,
			};
		})
		.toEqual({ firstIsDouble: true, nestedAreEqual: true, sameRow: true });

	const presentationByPanel = new Map<string, string>();
	const attachmentByPresentation = new Map<string, string>();
	const identityByPresentation = new Map<
		string,
		Awaited<ReturnType<typeof terminalDocumentIdentity>>
	>();
	for (const panelId of panelIds) {
		const presentationId = await panelFor(panelId)
			.locator('[data-slot="workbench-terminal-viewport"]')
			.getAttribute('data-terminal-presentation-id');
		if (presentationId === null) throw new Error('A Panel has no terminal presentation identity.');
		presentationByPanel.set(panelId, presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		const attachmentId = await terminalRoot.getAttribute('data-projected-terminal-attachment-id');
		if (attachmentId === null) throw new Error('A terminal has no projection attachment identity.');
		attachmentByPresentation.set(presentationId, attachmentId);
		identityByPresentation.set(presentationId, await terminalDocumentIdentity(terminalRoot));
	}

	const activeInput = focusedPanel.getByRole('textbox');
	await activeInput.focus();
	await activeInput.click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^(?:Equalize Panels|均分面板)/u }).click();
	await expect
		.poll(async () => {
			const widths = await Promise.all(
				panelIds.map(async (panelId) => (await panelRect(panelId)).width),
			);
			return Math.max(...widths) - Math.min(...widths);
		})
		.toBeLessThanOrEqual(4);
	await expect(activeInput).toBeFocused();
	const equalizedOrder = (
		await Promise.all(
			panelIds.map(async (panelId) => [panelId, (await panelRect(panelId)).x] as const),
		)
	)
		.sort((left, right) => left[1] - right[1])
		.map(([panelId]) => panelId);
	expect(equalizedOrder).toEqual(panelIds);

	await recordNextEqualizeDefault();
	await page.keyboard.press(equalizeShortcut);
	await expect(page.locator('html')).toHaveAttribute('data-fs14-key-default-prevented', 'true');
	await expect(activeInput).toBeFocused();

	await page.keyboard.press(maximizeShortcut);
	await recordNextEqualizeDefault();
	await page.keyboard.press(equalizeShortcut);
	await expect(page.locator('html')).toHaveAttribute('data-fs14-key-default-prevented', 'false');
	await page.keyboard.press(maximizeShortcut);
	await expect.poll(async () => (await panelRect(firstPanelId)).width).toBeGreaterThan(0);

	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '均分面板'
			: 'Equalize Panels',
	);
	await searchSurface.getByRole('option', { name: /(?:Equalize Panels|均分面板)/u }).click();
	await expect(searchSurface).not.toBeAttached();
	await expect(activeInput).toBeFocused();

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const equalizeShortcutRow = settingsSurface.locator(
		'[data-shortcut-id="workbench.panel.active.equalize"]',
	);
	await expect(equalizeShortcutRow).toContainText(/(?:Equalize panels|均分面板)/u);
	await expect(equalizeShortcutRow).toContainText(
		process.platform === 'darwin' ? '⌃⌘=' : 'Ctrl+Alt+=',
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();

	for (const [panelId, presentationId] of presentationByPanel) {
		await expect(
			panelFor(panelId).locator('[data-slot="workbench-terminal-viewport"]'),
		).toHaveAttribute('data-terminal-presentation-id', presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		const originalAttachmentId = attachmentByPresentation.get(presentationId);
		if (originalAttachmentId === undefined) throw new Error('Missing attachment identity witness.');
		await expect(terminalRoot).toHaveAttribute(
			'data-projected-terminal-attachment-id',
			originalAttachmentId,
		);
		const originalIdentity = identityByPresentation.get(presentationId);
		if (originalIdentity === undefined) throw new Error('Missing terminal identity witness.');
		expect(await terminalDocumentIdentity(terminalRoot)).toMatchObject({
			creationId: originalIdentity.creationId,
			terminalId: originalIdentity.terminalId,
		});
	}
	await expect(focusedPanel).toHaveAttribute('data-panel-id', thirdPanelId);
});

test('terminal-parity/FS-15-current-directory-tab', async () => {
	test.skip(
		process.platform === 'win32',
		'The exact current-directory witness uses a POSIX shell.',
	);
	test.setTimeout(60_000);
	const application = electronApplication;
	const directoryRoot = runtimeDirectory;
	if (application === undefined || directoryRoot === undefined) {
		throw new Error('Electron application did not start with a runtime directory.');
	}
	const page = await application.firstWindow();
	const currentDirectory = path.join(directoryRoot, 'fs15-current-directory');
	await mkdir(currentDirectory, { mode: 0o700 });
	const tabs = workbenchTabs(page);
	const sourceRoot = projectedTerminal(page);
	await expect(sourceRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const sourcePresentationId = await sourceRoot.getAttribute('data-terminal-presentation-id');
	if (sourcePresentationId === null) throw new Error('The source terminal has no presentation ID.');
	const sourceIdentity = await terminalDocumentIdentity(sourceRoot);
	const sourceInput = sourceRoot.getByRole('textbox');
	await sourceInput.focus();
	await sourceInput.pressSequentially(
		`cd ${quotePosixShellWord(currentDirectory)} && printf '\\033]7;file://%s%s\\007' "$(hostname)" "$PWD" && printf 'fs15-source-ready\\n'`,
		{ delay: 1 },
	);
	await sourceInput.press('Enter');
	await expect(sourceRoot.locator('[role="list"]')).toContainText('fs15-source-ready');
	await expect(tabs.first()).toContainText('fs15-current-directory');

	await sourceInput.click({ button: 'right' });
	const currentDirectoryMenuItem = page.getByRole('menuitem', {
		name: /^(?:New tab in current directory|在当前目录新建标签页)/u,
	});
	await expect(currentDirectoryMenuItem).toBeVisible();
	await expect(currentDirectoryMenuItem).toContainText(
		process.platform === 'darwin' ? '⌥⌘Enter' : 'Ctrl+Alt+Enter',
	);
	await page.keyboard.press('Escape');

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	await searchInput.fill(
		(await searchInput.getAttribute('placeholder')) === '搜索操作…'
			? '在当前目录新建标签页'
			: 'New tab in current directory',
	);
	await expect(
		searchSurface.getByRole('option', {
			name: /(?:New tab in current directory|在当前目录新建标签页)/u,
		}),
	).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(searchSurface).not.toBeAttached();

	await page.evaluate(() => {
		document.documentElement.removeAttribute('data-fs15-key-default-prevented');
		const record = (event: KeyboardEvent): void => {
			if (event.code !== 'Enter') return;
			document.documentElement.dataset['fs15KeyDefaultPrevented'] = String(event.defaultPrevented);
			document.removeEventListener('keydown', record, true);
		};
		document.addEventListener('keydown', record, true);
	});
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Alt+Enter' : 'Control+Alt+Enter');
	await expect(page.locator('html')).toHaveAttribute('data-fs15-key-default-prevented', 'true');
	await expect(tabs).toHaveCount(2);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	const createdRoot = page
		.locator(
			'[data-presentation-mode="active"] [data-terminal-presentation-id][data-projected-terminal-state]',
		)
		.first();
	await expect(createdRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const createdPresentationId = await createdRoot.getAttribute('data-terminal-presentation-id');
	if (createdPresentationId === null || createdPresentationId === sourcePresentationId) {
		throw new Error('The current-directory Action did not create a distinct terminal.');
	}
	const createdInput = createdRoot.getByRole('textbox');
	await expect(createdInput).toBeFocused();
	await createdInput.pressSequentially("pwd; printf 'fs15-created-ready\\n'", { delay: 1 });
	await createdInput.press('Enter');
	await expect(createdRoot.locator('[role="list"]')).toContainText(currentDirectory);
	await expect(createdRoot.locator('[role="list"]')).toContainText('fs15-created-ready');

	await tabs.first().click();
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	const restoredSourceRoot = projectedTerminalFor(page, sourcePresentationId);
	await expect(restoredSourceRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	expect(await terminalDocumentIdentity(restoredSourceRoot)).toMatchObject({
		creationId: sourceIdentity.creationId,
		terminalId: sourceIdentity.terminalId,
	});
	await expect(restoredSourceRoot.getByRole('textbox')).toBeFocused();

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const currentDirectoryShortcutRow = settingsSurface.locator(
		'[data-shortcut-id="workbench.terminal.create-current-directory"]',
	);
	await expect(currentDirectoryShortcutRow).toContainText(
		/(?:New terminal in current directory|在当前目录新建终端)/u,
	);
	await expect(currentDirectoryShortcutRow).toContainText(
		process.platform === 'darwin' ? '⌥⌘Enter' : 'Ctrl+Alt+Enter',
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
});

test('terminal-parity/FS-16-visible-panel-selection', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const activeLayer = page.locator('[data-presentation-mode="active"]');
	const workspace = page.locator('[data-slot="workbench-panel-workspace"]');
	const panels = activeLayer.locator('[data-slot="workbench-panel"]');
	const focusedPanel = activeLayer.locator('[data-slot="workbench-panel"][data-focused="true"]');
	const focusedPanelId = async (): Promise<string> => {
		const panelId = await focusedPanel.getAttribute('data-panel-id');
		if (panelId === null) throw new Error('The focused Panel has no stable identity.');
		return panelId;
	};
	const panelFor = (panelId: string): Locator =>
		activeLayer.locator(`[data-slot="workbench-panel"][data-panel-id="${panelId}"]`);
	const panelOrder = async (): Promise<readonly string[]> =>
		await panels.evaluateAll((elements) =>
			elements.map((element) => element.getAttribute('data-panel-id') ?? ''),
		);
	const selectionTargets = workspace.locator('[data-panel-visual-selection-target="true"]');

	const firstInput = projectedTerminal(page).getByRole('textbox');
	await firstInput.focus();
	const firstPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(2);
	const secondPanelId = await focusedPanelId();
	expect(secondPanelId).not.toBe(firstPanelId);

	await page.keyboard.press('Control+Shift+F7');
	await expect(selectionTargets).toHaveCount(0);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', firstPanelId);
	await expect(panelFor(firstPanelId).getByRole('textbox')).toBeFocused();

	await page.keyboard.press('Control+Shift+F8');
	await expect(selectionTargets).toHaveCount(0);
	await expect.poll(panelOrder).toEqual([secondPanelId, firstPanelId]);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', firstPanelId);
	await expect(panelFor(firstPanelId).getByRole('textbox')).toBeFocused();

	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(3);
	const thirdPanelId = await focusedPanelId();
	expect(new Set([firstPanelId, secondPanelId, thirdPanelId]).size).toBe(3);
	await expect.poll(panelOrder).toEqual([secondPanelId, firstPanelId, thirdPanelId]);

	const presentationByPanel = new Map<string, string>();
	const identityByPresentation = new Map<
		string,
		Awaited<ReturnType<typeof terminalDocumentIdentity>>
	>();
	for (const panelId of [firstPanelId, secondPanelId, thirdPanelId]) {
		const presentationId = await panelFor(panelId)
			.locator('[data-slot="workbench-terminal-viewport"]')
			.getAttribute('data-terminal-presentation-id');
		if (presentationId === null) throw new Error('A Panel has no terminal presentation identity.');
		presentationByPanel.set(panelId, presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		identityByPresentation.set(presentationId, await terminalDocumentIdentity(terminalRoot));
	}

	await page.keyboard.press('Control+Shift+F7');
	await expect(workspace).toHaveAttribute('data-panel-visual-selection-mode', 'focus');
	await expect(workspace).toHaveAttribute('data-workbench-keyboard-capture', 'true');
	await expect(workspace).toBeFocused();
	await expect(selectionTargets).toHaveCount(2);
	expect(
		await selectionTargets.evaluateAll((elements) =>
			elements.map((element) => ({
				label: element.getAttribute('data-label'),
				panelId: element.getAttribute('data-panel-id'),
			})),
		),
	).toEqual([
		{ label: '1', panelId: secondPanelId },
		{ label: '2', panelId: firstPanelId },
	]);
	await page.keyboard.press('Slash');
	await expect(workspace).toHaveAttribute('data-panel-visual-selection-mode', 'focus');
	await expect(workspace).toBeFocused();

	await page.keyboard.press('Control+Shift+Digit1');
	await expect(selectionTargets).toHaveCount(0);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', secondPanelId);
	await expect(panelFor(secondPanelId).getByRole('textbox')).toBeFocused();

	await page.keyboard.press('Control+Shift+F8');
	await expect(workspace).toHaveAttribute('data-panel-visual-selection-mode', 'swap');
	await expect(selectionTargets).toHaveCount(2);
	expect(
		await selectionTargets.evaluateAll((elements) =>
			elements.map((element) => ({
				label: element.getAttribute('data-label'),
				panelId: element.getAttribute('data-panel-id'),
			})),
		),
	).toEqual([
		{ label: '1', panelId: firstPanelId },
		{ label: '2', panelId: thirdPanelId },
	]);
	const pointerTarget = workspace.locator(
		`[data-panel-visual-selection-target="true"][data-panel-id="${thirdPanelId}"]`,
	);
	const pointerTargetBox = await pointerTarget.boundingBox();
	if (pointerTargetBox === null) throw new Error('The visual Panel target has no pointer box.');
	await page.mouse.click(
		pointerTargetBox.x + pointerTargetBox.width / 2,
		pointerTargetBox.y + pointerTargetBox.height / 2,
	);
	await expect(selectionTargets).toHaveCount(0);
	await expect.poll(panelOrder).toEqual([thirdPanelId, firstPanelId, secondPanelId]);
	await expect(focusedPanel).toHaveAttribute('data-panel-id', secondPanelId);
	await expect(panelFor(secondPanelId).getByRole('textbox')).toBeFocused();

	for (const [panelId, presentationId] of presentationByPanel) {
		await expect(
			panelFor(panelId).locator('[data-slot="workbench-terminal-viewport"]'),
		).toHaveAttribute('data-terminal-presentation-id', presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const originalIdentity = identityByPresentation.get(presentationId);
		if (originalIdentity === undefined) throw new Error('Missing terminal identity witness.');
		expect(await terminalDocumentIdentity(terminalRoot)).toMatchObject({
			creationId: originalIdentity.creationId,
			terminalId: originalIdentity.terminalId,
		});
	}

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await page.keyboard.press(`${primaryModifier}+Shift+KeyP`);
	const searchSurface = page.locator('[data-slot="workbench-search-surface"]');
	await expect(searchSurface).toBeVisible();
	const searchInput = searchSurface.getByRole('combobox');
	const chinese = (await searchInput.getAttribute('placeholder')) === '搜索操作…';
	await searchInput.fill(chinese ? '可视聚焦面板' : 'Visually Focus Panel');
	await expect(
		searchSurface.getByRole('option', { name: /(?:Visually Focus Panel|可视聚焦面板)/u }),
	).toBeVisible();
	await searchInput.fill(chinese ? '可视交换面板' : 'Visually Swap Panel');
	await expect(
		searchSurface.getByRole('option', { name: /(?:Visually Swap Panel|可视交换面板)/u }),
	).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(searchSurface).not.toBeAttached();

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const focusShortcutRow = settingsSurface.locator(
		'[data-shortcut-id="workbench.panel.active.focus-visible"]',
	);
	const swapShortcutRow = settingsSurface.locator(
		'[data-shortcut-id="workbench.panel.active.swap-visible"]',
	);
	await expect(focusShortcutRow).toContainText(/(?:Visually focus panel|可视聚焦面板)/u);
	await expect(focusShortcutRow).toContainText(
		process.platform === 'darwin' ? '⌃⇧F7' : 'Ctrl+Shift+F7',
	);
	await expect(swapShortcutRow).toContainText(/(?:Visually swap panel|可视交换面板)/u);
	await expect(swapShortcutRow).toContainText(
		process.platform === 'darwin' ? '⌃⇧F8' : 'Ctrl+Shift+F8',
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
});

test('terminal-parity/FS-17-panel-position-focus', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const maximizeShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+Enter' : 'Control+Shift+Enter';
	const activeLayer = page.locator('[data-presentation-mode="active"]');
	const workspace = page.locator('[data-slot="workbench-panel-workspace"]');
	const panels = activeLayer.locator('[data-slot="workbench-panel"]');
	const focusedPanel = activeLayer.locator('[data-slot="workbench-panel"][data-focused="true"]');
	const focusedPanelId = async (): Promise<string> => {
		const panelId = await focusedPanel.getAttribute('data-panel-id');
		if (panelId === null) throw new Error('The focused Panel has no stable identity.');
		return panelId;
	};
	const panelFor = (panelId: string): Locator =>
		activeLayer.locator(`[data-slot="workbench-panel"][data-panel-id="${panelId}"]`);
	const recordDefaultPrevention = async (code: string): Promise<void> => {
		await page.evaluate((expectedCode) => {
			document.documentElement.removeAttribute('data-fs17-key-default-prevented');
			const record = (event: KeyboardEvent): void => {
				if (event.code !== expectedCode) return;
				document.documentElement.dataset['fs17KeyDefaultPrevented'] = String(
					event.defaultPrevented,
				);
				document.removeEventListener('keydown', record, true);
			};
			document.addEventListener('keydown', record, true);
		}, code);
	};

	const firstInput = projectedTerminal(page).getByRole('textbox');
	await firstInput.focus();
	const firstPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(2);
	const secondPanelId = await focusedPanelId();
	await page.keyboard.press(splitRightShortcut);
	await expect(panels).toHaveCount(3);
	const thirdPanelId = await focusedPanelId();
	const canonicalPanelIds = [firstPanelId, secondPanelId, thirdPanelId] as const;
	expect(new Set(canonicalPanelIds).size).toBe(3);
	await expect
		.poll(
			async () =>
				await panels.evaluateAll((elements) =>
					elements.map((element) => element.getAttribute('data-panel-id') ?? ''),
				),
		)
		.toEqual(canonicalPanelIds);

	const presentationByPanel = new Map<string, string>();
	const identityByPresentation = new Map<
		string,
		Awaited<ReturnType<typeof terminalDocumentIdentity>>
	>();
	for (const panelId of canonicalPanelIds) {
		const presentationId = await panelFor(panelId)
			.locator('[data-slot="workbench-terminal-viewport"]')
			.getAttribute('data-terminal-presentation-id');
		if (presentationId === null) throw new Error('A Panel has no terminal presentation identity.');
		presentationByPanel.set(panelId, presentationId);
		const terminalRoot = projectedTerminalFor(page, presentationId);
		await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		identityByPresentation.set(presentationId, await terminalDocumentIdentity(terminalRoot));
	}

	for (const [position, panelId] of canonicalPanelIds.entries()) {
		const semanticPosition = position + 1;
		await recordDefaultPrevention(`Digit${semanticPosition}`);
		await page.keyboard.press(`Control+Shift+Digit${semanticPosition}`);
		await expect(page.locator('html')).toHaveAttribute('data-fs17-key-default-prevented', 'true');
		await expect(focusedPanel).toHaveAttribute('data-panel-id', panelId);
		await expect(panelFor(panelId).getByRole('textbox')).toBeFocused();
	}

	await recordDefaultPrevention('Digit3');
	await page.keyboard.press('Control+Shift+Digit3');
	await expect(page.locator('html')).toHaveAttribute('data-fs17-key-default-prevented', 'true');
	await expect(focusedPanel).toHaveAttribute('data-panel-id', thirdPanelId);
	await expect(panelFor(thirdPanelId).getByRole('textbox')).toBeFocused();

	await workspace.focus();
	await expect(workspace).toBeFocused();
	await recordDefaultPrevention('Digit0');
	await page.keyboard.press('Control+Shift+Digit0');
	await expect(page.locator('html')).toHaveAttribute('data-fs17-key-default-prevented', 'false');
	await expect(focusedPanel).toHaveAttribute('data-panel-id', thirdPanelId);

	await page.keyboard.press(maximizeShortcut);
	await expect(panelFor(firstPanelId)).toBeHidden();
	await expect(panelFor(secondPanelId)).toBeHidden();
	await expect(panelFor(thirdPanelId)).toBeVisible();
	await workspace.focus();
	await recordDefaultPrevention('Digit1');
	await page.keyboard.press('Control+Shift+Digit1');
	await expect(page.locator('html')).toHaveAttribute('data-fs17-key-default-prevented', 'false');
	await expect(focusedPanel).toHaveAttribute('data-panel-id', thirdPanelId);
	await page.keyboard.press(maximizeShortcut);
	for (const panelId of canonicalPanelIds) await expect(panelFor(panelId)).toBeVisible();

	for (const [panelId, presentationId] of presentationByPanel) {
		await expect(
			panelFor(panelId).locator('[data-slot="workbench-terminal-viewport"]'),
		).toHaveAttribute('data-terminal-presentation-id', presentationId);
		const originalIdentity = identityByPresentation.get(presentationId);
		if (originalIdentity === undefined) throw new Error('Missing terminal identity witness.');
		expect(
			await terminalDocumentIdentity(projectedTerminalFor(page, presentationId)),
		).toMatchObject({
			creationId: originalIdentity.creationId,
			terminalId: originalIdentity.terminalId,
		});
	}

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const firstPositionRow = settingsSurface.locator('[data-shortcut-id="workbench.panel.focus.1"]');
	const tenthPositionRow = settingsSurface.locator('[data-shortcut-id="workbench.panel.focus.10"]');
	await expect(firstPositionRow).toContainText(/(?:Focus panel 1|聚焦第 1 个面板)/u);
	await expect(firstPositionRow).toContainText(
		process.platform === 'darwin' ? '⌃⇧1' : 'Ctrl+Shift+1',
	);
	await expect(tenthPositionRow).toContainText(/(?:Focus panel 10|聚焦第 10 个面板)/u);
	await expect(tenthPositionRow).toContainText(
		process.platform === 'darwin' ? '⌃⇧0' : 'Ctrl+Shift+0',
	);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
});

test('keeps terminal presentation continuous through rapid Tab switching and active Tab closure', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const tabs = workbenchTabs(page);
	const newTabButton = page.getByRole('button', { name: /^(?:New tab|新建标签页)$/u });
	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';

	const firstRoot = projectedTerminal(page);
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-rendered-revision', /^\d+$/u);
	const firstPresentationId = await firstRoot.getAttribute('data-terminal-presentation-id');
	const firstAttachmentId = await firstRoot.getAttribute('data-projected-terminal-attachment-id');
	if (firstPresentationId === null || firstAttachmentId === null) {
		throw new Error('First Tab terminal projection identity is unavailable.');
	}

	await newTabButton.click();
	await expect(tabs).toHaveCount(2);
	const secondRoot = projectedTerminal(page);
	await expect(secondRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(secondRoot).toHaveAttribute('data-projected-terminal-rendered-revision', /^\d+$/u);
	const secondPresentationId = await secondRoot.getAttribute('data-terminal-presentation-id');
	const secondAttachmentId = await secondRoot.getAttribute('data-projected-terminal-attachment-id');
	if (secondPresentationId === null || secondAttachmentId === null) {
		throw new Error('Second Tab terminal projection identity is unavailable.');
	}

	for (let index = 0; index < 40; index += 1) {
		await page.keyboard.press(`${primaryModifier}+Digit1`);
		await page.keyboard.press(`${primaryModifier}+Digit2`);
	}

	await page.keyboard.press(`${primaryModifier}+Digit1`);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	const recoveredFirstRoot = projectedTerminalFor(page, firstPresentationId);
	await expect(recoveredFirstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(recoveredFirstRoot).toHaveAttribute(
		'data-projected-terminal-rendered-revision',
		/^\d+$/u,
	);
	expect(await recoveredFirstRoot.getAttribute('data-projected-terminal-attachment-id')).not.toBe(
		firstAttachmentId,
	);
	const recoveredFirstInput = recoveredFirstRoot.getByRole('textbox');
	await recoveredFirstInput.pressSequentially("printf 'rapid-tab-control-alive\\n'", { delay: 1 });
	await recoveredFirstInput.press('Enter');
	await expect(recoveredFirstRoot.locator('[role="list"]')).toContainText(
		'rapid-tab-control-alive',
	);

	await page.keyboard.press(`${primaryModifier}+Digit2`);
	await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
	const closingRoot = projectedTerminalFor(page, secondPresentationId);
	await expect(closingRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(closingRoot).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	await expect(closingRoot).toHaveAttribute('data-projected-terminal-rendered-revision', /^\d+$/u);
	expect(await closingRoot.getAttribute('data-projected-terminal-attachment-id')).not.toBe(
		secondAttachmentId,
	);

	const presentationTrace = await page.evaluateHandle(() => {
		const entries: Array<{
			readonly blankTop: boolean;
			readonly modes: readonly string[];
		}> = [];
		const control = { animationFrame: 0, running: true };
		const record = (): void => {
			const workspace = document.querySelector('[data-slot="workbench-panel-workspace"]');
			if (!(workspace instanceof HTMLElement)) {
				entries.push({ blankTop: true, modes: [] });
				return;
			}
			const layers = Array.from(workspace.children).filter(
				(element): element is HTMLElement =>
					element instanceof HTMLElement && element.dataset['presentationMode'] !== undefined,
			);
			const top =
				layers.find((layer) => layer.dataset['presentationMode'] === 'retained') ??
				layers.find((layer) => layer.dataset['presentationMode'] === 'incoming') ??
				layers.find((layer) => layer.dataset['presentationMode'] === 'active');
			const terminalRoots =
				top === undefined
					? []
					: Array.from(top.querySelectorAll('[data-terminal-projection-state]')).filter(
							(element): element is HTMLElement => element instanceof HTMLElement,
						);
			const meaningful =
				terminalRoots.length > 0 &&
				terminalRoots.every(
					(root) =>
						root.dataset['projectedTerminalCachedPresentation'] === 'true' ||
						root.dataset['projectedTerminalRenderedRevision'] !== undefined,
				);
			entries.push({
				blankTop: !meaningful,
				modes: layers.flatMap((layer) => layer.dataset['presentationMode'] ?? []),
			});
		};
		const observer = new MutationObserver(record);
		observer.observe(document.body, {
			attributeFilter: [
				'data-presentation-mode',
				'data-projected-terminal-cached-presentation',
				'data-projected-terminal-rendered-revision',
			],
			attributes: true,
			childList: true,
			subtree: true,
		});
		const sampleFrame = (): void => {
			if (!control.running) return;
			record();
			control.animationFrame = requestAnimationFrame(sampleFrame);
		};
		record();
		control.animationFrame = requestAnimationFrame(sampleFrame);
		return { control, entries, observer };
	});

	await tabs
		.nth(1)
		.getByRole('button', { name: /^(?:Close|关闭)$/u })
		.click();
	await expect(tabs).toHaveCount(1);
	await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
	await expect(recoveredFirstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(recoveredFirstRoot.locator('[role="list"]')).toContainText(
		'rapid-tab-control-alive',
	);

	const presentationEntries = await presentationTrace.evaluate((trace) => {
		trace.control.running = false;
		cancelAnimationFrame(trace.control.animationFrame);
		trace.observer.disconnect();
		return trace.entries;
	});
	await presentationTrace.dispose();
	expect(presentationEntries.length).toBeGreaterThan(1);
	expect(
		presentationEntries.some(
			(entry) => entry.modes.includes('incoming') && entry.modes.includes('retained'),
		),
	).toBe(true);
	expect(presentationEntries.filter((entry) => entry.blankTop)).toEqual([]);
});

test('keeps the macOS window open when Command+W repeat outruns terminal close', async () => {
	test.skip(process.platform !== 'darwin', 'The native Command+W accelerator is macOS-specific.');
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	await expect
		.poll(() =>
			application.evaluate(({ Menu }) =>
				Boolean(Menu.getApplicationMenu()?.getMenuItemById('desktop-window-close')),
			),
		)
		.toBe(true);
	expect(
		await application.evaluate(({ Menu }) => {
			const closeWindowItem = Menu.getApplicationMenu()?.getMenuItemById('desktop-window-close');
			return closeWindowItem === null || closeWindowItem === undefined
				? null
				: {
						accelerator: closeWindowItem.accelerator,
						registerAccelerator: closeWindowItem.registerAccelerator,
						role: closeWindowItem.role,
					};
		}),
	).toEqual({
		accelerator: 'CommandOrControl+W',
		registerAccelerator: false,
		role: 'close',
	});
	const tabTrack = page.locator(
		'[data-slot="workbench-tab-bar"][data-orientation="horizontal"] [data-horizontal-track]',
	);
	const tabs = workbenchTabs(page);
	const newTabButton = tabTrack.locator(':scope > button').last();
	await expect(projectedTerminal(page)).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(tabs).toHaveCount(1);
	const initialTabCount = await tabs.count();

	await newTabButton.click();
	await expect(tabs).toHaveCount(initialTabCount + 1);
	const activeTerminal = projectedTerminal(page);
	await expect(activeTerminal).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(activeTerminal).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	await activeTerminal.getByRole('textbox').focus();

	await page.keyboard.down('Meta');
	try {
		await page.keyboard.down('KeyW');
		await page.keyboard.down('KeyW');
		await page.keyboard.down('KeyW');
	} finally {
		await page.keyboard.up('KeyW').catch(() => undefined);
		await page.keyboard.up('Meta').catch(() => undefined);
	}

	await expect(tabs).toHaveCount(initialTabCount);
	expect(page.isClosed()).toBe(false);
	expect(
		await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length),
	).toBe(1);
});

async function runManualNativeWindowTest(): Promise<void> {
	test.setTimeout(60_000);
	const application = electronApplication;
	const currentRuntimeDirectory = runtimeDirectory;
	if (application === undefined) throw new Error('Electron application did not start.');
	if (currentRuntimeDirectory === undefined) {
		throw new Error('Desktop runtime directory is unavailable.');
	}
	const page = await application.firstWindow();
	const terminalRoot = projectedTerminal(page);
	await expect(terminalRoot)
		.toHaveAttribute('data-projected-terminal-state', 'open')
		.catch((error: unknown) => {
			const output = (applicationOutput.get(application) ?? []).join('');
			electronApplication = undefined;
			throw new Error(`Desktop window-action startup failed. ${output}`, { cause: error });
		});
	const attachmentId = await terminalRoot.getAttribute('data-projected-terminal-attachment-id');
	const terminalInput = page.getByRole('textbox');
	await terminalInput.pressSequentially("printf 'window-state-marker\\n'; sleep 300", {
		delay: 1,
	});
	await terminalInput.press('Enter');
	await expect(page.locator('[role="list"]')).toContainText('window-state-marker');
	await expect(terminalRoot).toHaveAttribute('data-projected-terminal-close-activity', 'active');
	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.minimize();
	});
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isMinimized()),
			),
		)
		.toBe(true);
	await application.evaluate(({ app }) => {
		app.emit('activate');
	});
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) => {
				const window = BrowserWindow.getAllWindows()[0];
				return {
					minimized: Boolean(window?.isMinimized()),
					visible: Boolean(window?.isVisible()),
				};
			}),
		)
		.toEqual({ minimized: false, visible: true });
	expect(await terminalRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);

	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-maximized', false);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isMaximized()),
			),
		)
		.toBe(true);
	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-maximized', true);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isMaximized()),
			),
		)
		.toBe(false);

	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-full-screen', true);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isFullScreen()),
			),
		)
		.toBe(true);
	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-maximized', true);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) => ({
				fullScreen: Boolean(BrowserWindow.getAllWindows()[0]?.isFullScreen()),
				maximized: Boolean(BrowserWindow.getAllWindows()[0]?.isMaximized()),
			})),
		)
		.toEqual({ fullScreen: true, maximized: false });
	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-full-screen', false);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isFullScreen()),
			),
		)
		.toBe(false);
	expect(await terminalRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);

	const windowGeometry = await application.evaluate(({ BrowserWindow, screen }) => {
		const workArea = screen.getPrimaryDisplay().workArea;
		const bounds = {
			height: Math.min(600, workArea.height),
			width: Math.min(900, workArea.width),
			x: workArea.x + 40,
			y: workArea.y + 40,
		};
		BrowserWindow.getAllWindows()[0]?.setBounds(bounds);
		return { primaryWorkArea: workArea, targetBounds: bounds };
	});
	const { primaryWorkArea, targetBounds } = windowGeometry;
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				BrowserWindow.getAllWindows()[0]?.getNormalBounds(),
			),
		)
		.toEqual(targetBounds);
	await invokeDesktopWindowMenuItem(application, 'desktop-window-toggle-maximized', false);
	await expect
		.poll(() =>
			application.evaluate(({ BrowserWindow }) =>
				Boolean(BrowserWindow.getAllWindows()[0]?.isMaximized()),
			),
		)
		.toBe(true);

	await quitDesktopWithChoice(application, page, 'preserve').catch((error: unknown) => {
		const output = (applicationOutput.get(application) ?? []).join('');
		electronApplication = undefined;
		throw new Error(`Desktop window-state shutdown failed. ${output}`, { cause: error });
	});
	electronApplication = undefined;
	const windowStatePath = path.join(
		currentRuntimeDirectory,
		'electron-user-data',
		'window-state-v1.json',
	);
	expect(JSON.parse(await readFile(windowStatePath, 'utf8'))).toEqual({
		bounds: targetBounds,
		maximized: true,
		version: 1,
	});

	const offscreenState = {
		bounds: { height: 600, width: 900, x: 999_999, y: 999_999 },
		maximized: true,
		version: 1,
	} as const;
	await writeFile(windowStatePath, `${JSON.stringify(offscreenState)}\n`, 'utf8');
	const expectedBounds = {
		height: Math.min(offscreenState.bounds.height, primaryWorkArea.height),
		width: Math.min(offscreenState.bounds.width, primaryWorkArea.width),
		x:
			primaryWorkArea.x +
			primaryWorkArea.width -
			Math.min(offscreenState.bounds.width, primaryWorkArea.width),
		y:
			primaryWorkArea.y +
			primaryWorkArea.height -
			Math.min(offscreenState.bounds.height, primaryWorkArea.height),
	};

	const restartedApplication = await launchDesktop();
	electronApplication = restartedApplication;
	const restartedPage = await restartedApplication.firstWindow();
	await expect(projectedTerminal(restartedPage)).toHaveAttribute(
		'data-projected-terminal-state',
		'open',
	);
	await expect
		.poll(() =>
			restartedApplication.evaluate(({ BrowserWindow }) => ({
				bounds: BrowserWindow.getAllWindows()[0]?.getNormalBounds(),
				maximized: Boolean(BrowserWindow.getAllWindows()[0]?.isMaximized()),
			})),
		)
		.toEqual({ bounds: expectedBounds, maximized: true });
}

test(
	'owns native window actions and restores safe normal bounds across restart',
	{ tag: MANUAL_NATIVE_WINDOW_TEST_TAG },
	runManualNativeWindowTest,
);

test('applies a custom shortcut without recreating the mounted terminal', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const terminalRoot = projectedTerminal(page);
	await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await terminalRoot.getAttribute('data-projected-terminal-attachment-id');
	const leftPanel = page.locator('[data-slot="workbench-left-panel"]');
	await expect(leftPanel).toBeVisible();

	const settingsSurface = await openDesktopSettingsCategory(page, 'keyboard-shortcuts');
	const shortcutRow = settingsSurface.locator('[data-shortcut-id="workbench.sidebar.toggle"]');
	const recorder = shortcutRow.locator('button[aria-pressed]');
	await recorder.click();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Alt+KeyB' : 'Control+Alt+KeyB');
	await expect(shortcutRow.locator('[data-source="custom"]')).toBeVisible();
	await settingsSurface.locator('aside > button').first().click();
	await expect(settingsSurface).not.toBeVisible();
	expect(await terminalRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Alt+KeyB' : 'Control+Alt+KeyB');
	await expect(leftPanel).not.toBeAttached();
	expect(await terminalRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);
});

test('terminal-parity/SU-18-font-size-shortcuts', async () => {
	test.setTimeout(45_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const terminals = projectedTerminals(page);
	await expect(terminals).toHaveCount(1);
	await expect(terminals.first()).toHaveAttribute('data-projected-terminal-state', 'open');

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD');
	await expect(terminals).toHaveCount(2);
	const attachmentIds = await terminals.evaluateAll((elements) =>
		elements.map((element) => element.getAttribute('data-projected-terminal-attachment-id')),
	);
	const fontSizes = async (): Promise<readonly number[]> =>
		await terminals.locator('textarea').evaluateAll((elements) =>
			elements.map((element) => {
				if (!(element instanceof HTMLTextAreaElement)) {
					throw new Error('Projected terminal input witness is not a textarea.');
				}
				return Number.parseFloat(element.style.fontSize);
			}),
		);
	const initialFontSizes = await fontSizes();
	expect(initialFontSizes).toHaveLength(2);
	expect(new Set(initialFontSizes).size).toBe(1);

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Equal' : 'Control+Equal');
	await expect.poll(fontSizes).toEqual(initialFontSizes.map((fontSize) => fontSize + 2));
	expect(
		await terminals.evaluateAll((elements) =>
			elements.map((element) => element.getAttribute('data-projected-terminal-attachment-id')),
		),
	).toEqual(attachmentIds);

	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Minus' : 'Control+Minus');
	await expect.poll(fontSizes).toEqual(initialFontSizes);
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Equal' : 'Control+Equal');
	await expect.poll(fontSizes).toEqual(initialFontSizes.map((fontSize) => fontSize + 2));
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+Digit0' : 'Control+Digit0');
	await expect.poll(fontSizes).toEqual(initialFontSizes);
});

test('launches only future terminals with the committed local shell profile', async () => {
	test.skip(process.platform === 'win32', 'The native witness uses a POSIX shell executable.');
	test.setTimeout(45_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const initialTerminal = projectedTerminal(page);
	await expect(initialTerminal).toHaveAttribute('data-projected-terminal-state', 'open');
	const initialPresentationId = await initialTerminal.getAttribute('data-terminal-presentation-id');
	const initialAttachmentId = await initialTerminal.getAttribute(
		'data-projected-terminal-attachment-id',
	);

	const settingsSurface = await openDesktopSettingsCategory(page, 'profiles');
	await settingsSurface.locator('#terminal-profile-command').selectOption('custom-executable');
	await settingsSurface.locator('#terminal-profile-executable').fill('/bin/sh');
	const addArgument = settingsSurface.locator('[data-slot="terminal-launch-profile-add-argument"]');
	await addArgument.click();
	await addArgument.click();
	const argumentsInputs = settingsSurface.locator('[id="profiles.arguments"] input');
	await expect(argumentsInputs).toHaveCount(2);
	await argumentsInputs.nth(0).fill('-c');
	await argumentsInputs.nth(1).fill("printf 'local-shell-profile-v1-marker\\n'; sleep 30");
	await settingsSurface.locator('[data-slot="terminal-launch-profile-save"]').click();

	const userDataPath = await application.evaluate(({ app }) => app.getPath('userData'));
	const profileStorePath = path.join(userDataPath, 'terminal-launch-profiles-v1.json');
	await expect
		.poll(() => readFile(profileStorePath, 'utf8').catch(() => ''))
		.toContain('local-shell-profile-v1-marker');
	await settingsSurface.locator('aside > button').first().click();
	await expect(settingsSurface).not.toBeVisible();
	expect(await initialTerminal.getAttribute('data-terminal-presentation-id')).toBe(
		initialPresentationId,
	);
	expect(await initialTerminal.getAttribute('data-projected-terminal-attachment-id')).toBe(
		initialAttachmentId,
	);

	await page.getByRole('button', { name: /^(?:New tab|新建标签页)$/u }).click();
	await expect(workbenchTabs(page)).toHaveCount(2);
	const profileTerminal = projectedTerminal(page);
	await expect(profileTerminal).toHaveAttribute('data-projected-terminal-state', 'open');
	expect(await profileTerminal.getAttribute('data-terminal-presentation-id')).not.toBe(
		initialPresentationId,
	);
	await expect(profileTerminal.locator('[role="list"]')).toContainText(
		'local-shell-profile-v1-marker',
	);
});

test('terminal-parity/IN-08-drop', async () => {
	test.skip(process.platform === 'win32', 'The accepted first slice supports POSIX shells only.');
	const application = electronApplication;
	const directory = runtimeDirectory;
	if (application === undefined || directory === undefined) {
		throw new Error('Desktop file-drop fixture is unavailable.');
	}
	const page = await application.firstWindow();
	const terminalRoot = projectedTerminal(page);
	await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const scriptPath = path.join(directory, 'desktop-file-drop.sh');
	await writeFile(scriptPath, "#!/bin/sh\nprintf 'desktop-file-drop-executed\\n'\n", 'utf8');
	await chmod(scriptPath, 0o700);
	await page.evaluate(() => {
		const input = document.createElement('input');
		input.dataset['desktopFileDropFixture'] = 'true';
		input.type = 'file';
		document.body.appendChild(input);
	});
	await page.locator('input[data-desktop-file-drop-fixture="true"]').setInputFiles(scriptPath);

	const input = page.getByRole('textbox');
	const prefix = 'drop-before-';
	const suffix = '-drop-after';
	await input.pressSequentially(`${prefix}${suffix}`, { delay: 1 });
	for (let index = 0; index < suffix.length; index += 1) {
		await input.press('ArrowLeft');
	}
	const firstDropTrace = await dispatchDesktopFileDrop(terminalRoot);
	expect(firstDropTrace).toEqual({
		bridgePath: scriptPath,
		dragOverPrevented: true,
		dropFileCount: 1,
		dropItemCount: 1,
		dropPrevented: true,
		dropTypes: ['Files'],
	});
	const review = page.locator('[data-slot="projected-terminal-paste-review"]');
	await expect(review).not.toBeVisible();
	await expect(page.locator('[role="list"]')).toContainText(`${prefix}${scriptPath}${suffix}`);
	await page.waitForTimeout(300);
	await expect(page.locator('[role="list"]')).not.toContainText('desktop-file-drop-executed');
	await input.press('Control+C');

	const secondDropTrace = await dispatchDesktopFileDrop(terminalRoot);
	expect(secondDropTrace).toEqual(firstDropTrace);
	await expect(page.locator('[role="list"]')).toContainText(scriptPath);
	await page.waitForTimeout(300);
	await expect(page.locator('[role="list"]')).not.toContainText('desktop-file-drop-executed');

	await input.press('Enter');
	await expect(page.locator('[role="list"]')).toContainText('desktop-file-drop-executed');
});

test(launchEnvironmentWitnessTitle, async () => {
	test.skip(process.platform === 'win32', 'The launch-locale witness is POSIX-only.');
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');

	const input = root.getByRole('textbox');
	const command =
		'printf \'orchbee-launch-env LANG=%s TERM=%s COLORTERM=%s TERM_PROGRAM=%s NO_COLOR=%s FORCE_COLOR=%s NODE_DISABLE_COLORS=%s CHARMAP=%s\\n\' "$LANG" "$TERM" "$COLORTERM" "$TERM_PROGRAM" "${NO_COLOR-unset}" "${FORCE_COLOR-unset}" "${NODE_DISABLE_COLORS-unset}" "$(locale charmap 2>/dev/null || printf unknown)"';
	await input.pressSequentially(command, { delay: 1 });
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText(
		/orchbee-launch-env LANG=[^ ]+\.UTF-?8 TERM=xterm-256color COLORTERM=truecolor TERM_PROGRAM=ghostty NO_COLOR=unset FORCE_COLOR=unset NODE_DISABLE_COLORS=unset CHARMAP=UTF-?8/u,
	);
});

test(sshEnvironmentWitnessTitle, async () => {
	test.skip(process.platform !== 'darwin', 'The current real SSH baseline is verified on macOS.');
	test.setTimeout(45_000);
	const application = electronApplication;
	const directory = runtimeDirectory;
	if (application === undefined || directory === undefined) {
		throw new Error('Desktop remote-development fixture is unavailable.');
	}
	const page = await application.firstWindow();
	const terminalRoot = projectedTerminal(page);
	await expect(terminalRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const settingsSurface = await openDesktopSettingsCategory(page, 'remote-development');
	const diagnostic = settingsSurface.locator('#remote-development-diagnostic-command');
	await expect(diagnostic).toHaveJSProperty('readOnly', true);
	await expect(diagnostic).toHaveValue(/ORCHBEE_REMOTE_DIAGNOSTIC_V1.*infocmp/su);
	const diagnosticCommand = await diagnostic.inputValue();
	await expect(settingsSurface).toContainText(/No automatic remote changes|不会自动修改远端/u);
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();

	const sshFixture = await startLoopbackSshFixture(directory);
	try {
		const sshCommand = [
			'/usr/bin/ssh',
			'-F',
			'/dev/null',
			'-tt',
			'-o',
			'BatchMode=yes',
			'-o',
			'IdentitiesOnly=yes',
			'-o',
			'LogLevel=ERROR',
			'-o',
			'StrictHostKeyChecking=no',
			'-o',
			'UserKnownHostsFile=/dev/null',
			'-i',
			sshFixture.clientKey,
			'-p',
			String(sshFixture.port),
			`${sshFixture.username}@127.0.0.1`,
			diagnosticCommand,
		]
			.map(quotePosixShellWord)
			.join(' ');
		const input = page.getByRole('textbox');
		await input.pressSequentially(sshCommand, { delay: 1 });
		await input.press('Enter');
		const accessibility = page.locator('[role="list"]');
		await expect(accessibility).toContainText('ORCHBEE_REMOTE_DIAGNOSTIC_V1');
		await expect(accessibility).toContainText('TERM=xterm-256color');
		await expect(accessibility).toContainText('COLORTERM=unset');
		await expect(accessibility).toContainText('TERM_PROGRAM=unset');
		await expect(accessibility).toContainText('TERMINFO=available');

		const localeCommand =
			'printf \'ORCHBEE_REMOTE_LOCALE LANG=%s CHARMAP=%s\\n\' "${LANG-unset}" "$(locale charmap 2>/dev/null || printf unknown)"';
		const localeSshCommand = [
			'/usr/bin/ssh',
			'-F',
			'/dev/null',
			'-tt',
			'-o',
			'BatchMode=yes',
			'-o',
			'IdentitiesOnly=yes',
			'-o',
			'LogLevel=ERROR',
			'-o',
			'SendEnv=LANG',
			'-o',
			'StrictHostKeyChecking=no',
			'-o',
			'UserKnownHostsFile=/dev/null',
			'-i',
			sshFixture.clientKey,
			'-p',
			String(sshFixture.port),
			`${sshFixture.username}@127.0.0.1`,
			localeCommand,
		]
			.map(quotePosixShellWord)
			.join(' ');
		await input.pressSequentially(localeSshCommand, { delay: 1 });
		await input.press('Enter');
		await expect(accessibility).toContainText(
			/ORCHBEE_REMOTE_LOCALE LANG=[^ ]+\.UTF-?8 CHARMAP=UTF-?8/u,
		);
	} finally {
		await stopChildProcess(sshFixture.server);
	}
});

test('closes the final Tab after a natural Host terminal exit while keeping the window open', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	await expect(projectedTerminal(page)).toHaveAttribute('data-projected-terminal-state', 'open');

	const input = page.getByRole('textbox');
	await input.pressSequentially('exit', { delay: 1 });
	await input.press('Enter');

	await expect(workbenchTabs(page)).toHaveCount(0);
	await expect(projectedTerminals(page)).toHaveCount(0);
	expect(
		await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length),
	).toBe(1);
});

test('moves terminal focus through a four-Panel layout by visual direction', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryRoot = projectedTerminal(page);
	await expect(primaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const primaryPresentationId = await primaryRoot.getAttribute('data-terminal-presentation-id');
	if (primaryPresentationId === null) {
		throw new Error('Primary terminal presentation identity is unavailable.');
	}

	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const splitBelowShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+KeyD' : 'Control+Shift+KeyE';
	const focusModifier = process.platform === 'darwin' ? 'Meta+Alt' : 'Control+Alt';
	await primaryRoot.getByRole('textbox').focus();
	await page.keyboard.press(splitRightShortcut);
	await expect(projectedTerminals(page)).toHaveCount(2);
	await waitForProjectedTerminalsOpen(page);
	const rightPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) => presentationId !== primaryPresentationId,
	);
	if (rightPresentationId === undefined) {
		throw new Error('Right terminal presentation identity is unavailable.');
	}
	const rightRoot = projectedTerminalFor(page, rightPresentationId);
	await expect(rightRoot.getByRole('textbox')).toBeFocused();

	await page.keyboard.press(splitBelowShortcut);
	await expect(projectedTerminals(page)).toHaveCount(3);
	await waitForProjectedTerminalsOpen(page);
	const bottomRightPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) =>
			presentationId !== primaryPresentationId && presentationId !== rightPresentationId,
	);
	if (bottomRightPresentationId === undefined) {
		throw new Error('Bottom-right terminal presentation identity is unavailable.');
	}
	const bottomRightRoot = projectedTerminalFor(page, bottomRightPresentationId);
	await expect(bottomRightRoot.getByRole('textbox')).toBeFocused();

	await page.keyboard.press(`${focusModifier}+ArrowLeft`);
	await expect(primaryRoot.getByRole('textbox')).toBeFocused();
	await page.keyboard.press(splitBelowShortcut);
	await expect(projectedTerminals(page)).toHaveCount(4);
	await waitForProjectedTerminalsOpen(page);
	const bottomLeftPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) =>
			presentationId !== primaryPresentationId &&
			presentationId !== rightPresentationId &&
			presentationId !== bottomRightPresentationId,
	);
	if (bottomLeftPresentationId === undefined) {
		throw new Error('Bottom-left terminal presentation identity is unavailable.');
	}
	const bottomLeftRoot = projectedTerminalFor(page, bottomLeftPresentationId);
	await expect(bottomLeftRoot.getByRole('textbox')).toBeFocused();

	await page.keyboard.press(`${focusModifier}+ArrowUp`);
	await expect(primaryRoot.getByRole('textbox')).toBeFocused();
	await page.keyboard.press(`${focusModifier}+ArrowRight`);
	await expect(rightRoot.getByRole('textbox')).toBeFocused();
	await page.keyboard.press(`${focusModifier}+ArrowDown`);
	await expect(bottomRightRoot.getByRole('textbox')).toBeFocused();
	await page.keyboard.press(`${focusModifier}+ArrowLeft`);
	await expect(bottomLeftRoot.getByRole('textbox')).toBeFocused();
	await page.keyboard.press(`${focusModifier}+ArrowDown`);
	await expect(bottomLeftRoot.getByRole('textbox')).toBeFocused();
});

test('restores terminal geometry, focus, input, and scrolling after Panel maximization', async () => {
	test.setTimeout(45_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryRoot = projectedTerminal(page);
	await expect(primaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const primaryPresentationId = await primaryRoot.getAttribute('data-terminal-presentation-id');
	if (primaryPresentationId === null) {
		throw new Error('Primary terminal presentation identity is unavailable.');
	}

	await primaryRoot.getByRole('textbox').focus();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD');
	await expect(projectedTerminals(page)).toHaveCount(2);
	await waitForProjectedTerminalsOpen(page);
	const splitPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) => presentationId !== primaryPresentationId,
	);
	if (splitPresentationId === undefined) {
		throw new Error('Split terminal presentation identity is unavailable.');
	}
	const splitRoot = page.locator(
		`[data-presentation-mode="active"] [data-terminal-presentation-id="${splitPresentationId}"][data-projected-terminal-state]`,
	);
	const splitInput = splitRoot.getByRole('textbox');
	const splitPanel = splitRoot.locator('xpath=ancestor::*[@data-slot="workbench-panel"][1]');
	await expect(splitInput).toBeFocused();

	const readGeometry = async (): Promise<{
		readonly canvasWidth: number;
		readonly columns: number;
		readonly panelWidth: number;
		readonly rootWidth: number;
	}> => {
		const [surface, panelBounds] = await Promise.all([
			splitRoot.evaluate((element) => {
				const canvas = element.querySelector('canvas');
				if (!(canvas instanceof HTMLCanvasElement)) {
					throw new Error('Split terminal canvas is unavailable.');
				}
				return {
					canvasWidth: canvas.getBoundingClientRect().width,
					columns: Number(element.dataset['projectedTerminalColumns'] ?? '0'),
					rootWidth: element.getBoundingClientRect().width,
				};
			}),
			splitPanel.boundingBox(),
		]);
		if (panelBounds === null) throw new Error('Split Panel geometry is unavailable.');
		return { ...surface, panelWidth: panelBounds.width };
	};
	const initialGeometry = await readGeometry();
	expect(initialGeometry.columns).toBeGreaterThan(0);
	await expect(splitRoot).toHaveAttribute(
		'data-projected-terminal-presentation-geometry-current',
		'true',
	);

	await startTerminalPresentationFrameTrace(page);
	await splitPanel.getByRole('button', { name: /Panel options|面板选项/u }).click();
	await page.getByRole('menuitem', { name: /Maximize Panel|最大化面板/u }).click();
	await expect(splitRoot).toHaveAttribute(
		'data-projected-terminal-presentation-geometry-current',
		'true',
	);
	await expect(splitInput).toBeFocused();
	const maximizedGeometry = await readGeometry();
	expect(maximizedGeometry.columns).toBeGreaterThan(initialGeometry.columns);
	expect(maximizedGeometry.panelWidth).toBeGreaterThan(initialGeometry.panelWidth);
	expect(maximizedGeometry.rootWidth).toBeGreaterThan(initialGeometry.rootWidth);
	expect(maximizedGeometry.canvasWidth).toBeGreaterThan(initialGeometry.canvasWidth);
	expectAtomicTerminalPresentationTrace(await stopTerminalPresentationFrameTrace(page));

	await splitInput.pressSequentially("seq 180; printf 'panel-maximized-marker\\n'", { delay: 1 });
	await splitInput.press('Enter');
	const accessibility = splitRoot.locator('[role="list"]');
	await expect(accessibility).toContainText('panel-maximized-marker');
	await expect
		.poll(async () => Number(await accessibility.getAttribute('data-terminal-retained-rows')))
		.toBeGreaterThan(180);

	await startTerminalPresentationFrameTrace(page);
	await splitPanel.getByRole('button', { name: /Panel options|面板选项/u }).click();
	await page.getByRole('menuitem', { name: /Minimize Panel|最小化面板/u }).click();
	await expect(splitRoot).toHaveAttribute(
		'data-projected-terminal-presentation-geometry-current',
		'true',
	);
	await expect(splitInput).toBeFocused();
	const restoredGeometry = await readGeometry();
	expect(restoredGeometry.columns).toBe(initialGeometry.columns);
	expect(Math.abs(restoredGeometry.panelWidth - initialGeometry.panelWidth)).toBeLessThanOrEqual(2);
	expect(Math.abs(restoredGeometry.rootWidth - initialGeometry.rootWidth)).toBeLessThanOrEqual(2);
	expect(Math.abs(restoredGeometry.canvasWidth - initialGeometry.canvasWidth)).toBeLessThanOrEqual(
		2,
	);
	expect(restoredGeometry.canvasWidth).toBeLessThan(maximizedGeometry.canvasWidth);
	expectAtomicTerminalPresentationTrace(await stopTerminalPresentationFrameTrace(page));

	await splitInput.pressSequentially("printf 'panel-restored-input-marker\\n'", { delay: 1 });
	await splitInput.press('Enter');
	await expect(accessibility).toContainText('panel-restored-input-marker');
	await waitForTerminalPresentationStability(splitRoot);
	const accessibleRows = accessibility.locator(':scope > div');
	const firstVisibleRow = async (): Promise<bigint> =>
		BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0');
	const bottomStart = await firstVisibleRow();
	const restoredBounds = await splitRoot.boundingBox();
	if (restoredBounds === null) throw new Error('Restored terminal surface has no bounds.');
	await splitRoot.locator('canvas').hover();
	await page.mouse.wheel(0, -restoredBounds.height);
	await expect.poll(firstVisibleRow).toBeLessThan(bottomStart);
});

test('restores an equal four-Panel grid and the MRU focus after closing a nested split', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryRoot = projectedTerminal(page);
	await expect(primaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const primaryPresentationId = await primaryRoot.getAttribute('data-terminal-presentation-id');
	if (primaryPresentationId === null) {
		throw new Error('Primary terminal presentation identity is unavailable.');
	}

	const splitRightShortcut = process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD';
	const splitBelowShortcut =
		process.platform === 'darwin' ? 'Meta+Shift+KeyD' : 'Control+Shift+KeyE';
	const focusModifier = process.platform === 'darwin' ? 'Meta+Alt' : 'Control+Alt';
	const closeShortcut = process.platform === 'darwin' ? 'Meta+KeyW' : 'Control+Shift+KeyW';
	await primaryRoot.getByRole('textbox').focus();
	await page.keyboard.press(splitBelowShortcut);
	await expect(projectedTerminals(page)).toHaveCount(2);
	await waitForProjectedTerminalsOpen(page);
	const bottomLeftPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) => presentationId !== primaryPresentationId,
	);
	if (bottomLeftPresentationId === undefined) {
		throw new Error('Bottom-left terminal presentation identity is unavailable.');
	}

	await page.keyboard.press(`${focusModifier}+ArrowUp`);
	await page.keyboard.press(splitRightShortcut);
	await expect(projectedTerminals(page)).toHaveCount(3);
	await waitForProjectedTerminalsOpen(page);
	const topRightPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) =>
			presentationId !== primaryPresentationId && presentationId !== bottomLeftPresentationId,
	);
	if (topRightPresentationId === undefined) {
		throw new Error('Top-right terminal presentation identity is unavailable.');
	}

	await page.keyboard.press(`${focusModifier}+ArrowDown`);
	await page.keyboard.press(splitRightShortcut);
	await expect(projectedTerminals(page)).toHaveCount(4);
	await waitForProjectedTerminalsOpen(page);
	const nestedLeftPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) =>
			presentationId !== primaryPresentationId &&
			presentationId !== bottomLeftPresentationId &&
			presentationId !== topRightPresentationId,
	);
	if (nestedLeftPresentationId === undefined) {
		throw new Error('Nested-left terminal presentation identity is unavailable.');
	}

	await page.keyboard.press(splitRightShortcut);
	await expect(projectedTerminals(page)).toHaveCount(5);
	await waitForProjectedTerminalsOpen(page);
	const bottomRightPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) =>
			presentationId !== primaryPresentationId &&
			presentationId !== bottomLeftPresentationId &&
			presentationId !== topRightPresentationId &&
			presentationId !== nestedLeftPresentationId,
	);
	if (bottomRightPresentationId === undefined) {
		throw new Error('Bottom-right terminal presentation identity is unavailable.');
	}

	const panelFor = (presentationId: string): Locator =>
		page.locator('[data-slot="workbench-panel"]').filter({
			has: projectedTerminalFor(page, presentationId),
		});
	const bottomLeftPanel = panelFor(bottomLeftPresentationId);
	const nestedLeftPanel = panelFor(nestedLeftPresentationId);
	const bottomRightPanel = panelFor(bottomRightPresentationId);
	const beforeCloseBounds = await Promise.all([
		bottomLeftPanel.boundingBox(),
		nestedLeftPanel.boundingBox(),
		bottomRightPanel.boundingBox(),
	]);
	if (beforeCloseBounds.some((bounds) => bounds === null)) {
		throw new Error('Pre-close Panel geometry is unavailable.');
	}
	const [bottomLeftBefore, nestedLeftBefore, bottomRightBefore] = beforeCloseBounds;
	if (bottomLeftBefore === null || nestedLeftBefore === null || bottomRightBefore === null) {
		throw new Error('Pre-close Panel geometry is unavailable.');
	}
	expect(bottomLeftBefore.width).toBeGreaterThan(nestedLeftBefore.width * 1.8);
	expect(Math.abs(nestedLeftBefore.width - bottomRightBefore.width)).toBeLessThanOrEqual(2);

	await page.keyboard.press(`${focusModifier}+ArrowLeft`);
	const nestedLeftRoot = projectedTerminalFor(page, nestedLeftPresentationId);
	await expect(nestedLeftRoot.getByRole('textbox')).toBeFocused();
	await expect(nestedLeftRoot).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	await page.keyboard.press(closeShortcut);
	await expect(nestedLeftRoot).toHaveCount(0);
	await expect(projectedTerminals(page)).toHaveCount(4);
	await expect(bottomRightPanel).toHaveAttribute('data-focused', 'true');
	await expect(
		projectedTerminalFor(page, bottomRightPresentationId).getByRole('textbox'),
	).toBeFocused();

	const fourPanelBounds = await Promise.all([
		panelFor(primaryPresentationId).boundingBox(),
		panelFor(topRightPresentationId).boundingBox(),
		panelFor(bottomLeftPresentationId).boundingBox(),
		panelFor(bottomRightPresentationId).boundingBox(),
	]);
	if (fourPanelBounds.some((bounds) => bounds === null)) {
		throw new Error('Post-close four-Panel geometry is unavailable.');
	}
	const [topLeft, topRight, bottomLeft, bottomRight] = fourPanelBounds;
	if (topLeft === null || topRight === null || bottomLeft === null || bottomRight === null) {
		throw new Error('Post-close four-Panel geometry is unavailable.');
	}
	for (const bounds of [topRight, bottomLeft, bottomRight]) {
		expect(Math.abs(bounds.width - topLeft.width)).toBeLessThanOrEqual(2);
		expect(Math.abs(bounds.height - topLeft.height)).toBeLessThanOrEqual(2);
	}
	expect(Math.abs(bottomLeft.x - topLeft.x)).toBeLessThanOrEqual(2);
	expect(Math.abs(bottomRight.x - topRight.x)).toBeLessThanOrEqual(2);
});

test('restores the focused split terminal after a renderer reload', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const primaryRoot = projectedTerminal(page);
	await expect(primaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const primaryPresentationId = await primaryRoot.getAttribute('data-terminal-presentation-id');
	if (primaryPresentationId === null) {
		throw new Error('Primary terminal presentation identity is unavailable.');
	}

	await primaryRoot.getByRole('textbox').focus();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyD' : 'Control+Shift+KeyD');
	await expect(projectedTerminals(page)).toHaveCount(2);
	await waitForProjectedTerminalsOpen(page);
	const splitPresentationId = (await projectedTerminalPresentationIds(page)).find(
		(presentationId) => presentationId !== primaryPresentationId,
	);
	if (splitPresentationId === undefined) {
		throw new Error('Split terminal presentation identity is unavailable.');
	}
	const splitRoot = projectedTerminalFor(page, splitPresentationId);
	await expect(splitRoot.getByRole('textbox')).toBeFocused();

	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.webContents.reload();
	});
	await expect(projectedTerminals(page)).toHaveCount(2);
	await waitForProjectedTerminalsOpen(page);
	await expect(splitRoot.getByRole('textbox')).toBeFocused();

	await page.keyboard.insertText("printf 'renderer-reload-focus-target\\n'");
	await page.keyboard.press('Enter');
	await expect(splitRoot.locator('[role="list"]')).toContainText('renderer-reload-focus-target');
	await expect(primaryRoot.locator('[role="list"]')).not.toContainText(
		'renderer-reload-focus-target',
	);
});

test('routes a real sandboxed Host terminal through Main and Preload to the renderer', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const traceLines: string[] = [];
	application.process().stdout?.on('data', (chunk: Buffer) => traceLines.push(chunk.toString()));
	const page = await application.firstWindow();
	expect(new URL(page.url()).searchParams.get('orchbee-runtime')).toBeNull();
	let workerStarts = 0;
	page.on('worker', () => {
		workerStarts += 1;
	});
	const applicationRoot = page.locator('#root');
	const root = projectedTerminal(page);
	await expect(applicationRoot).toHaveAttribute('data-desktop-context-isolated', 'true');
	await expect(applicationRoot).toHaveAttribute('data-desktop-sandboxed', 'true');
	await expect(page.locator('[data-terminal-projection-state]').first()).toHaveAttribute(
		'data-terminal-projection-state',
		'requested',
	);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	await expectTerminalSurfaceAppearance(root);
	const rendererAuthority = await page.evaluate(() => ({
		hasGenericOrchbeeApi: 'orchbee' in globalThis,
		hasNodeRequire: 'require' in globalThis,
	}));
	expect(rendererAuthority).toEqual({ hasGenericOrchbeeApi: false, hasNodeRequire: false });

	const initialWindowColumns = Number(await root.getAttribute('data-projected-terminal-columns'));
	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.setSize(1_200, 650);
	});
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-columns')))
		.toBeGreaterThan(initialWindowColumns);
	const visibleSidebarColumns = Number(await root.getAttribute('data-projected-terminal-columns'));
	await page.locator('[data-slot="workbench-sidebar-toggle"]').click();
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-columns')))
		.toBeGreaterThan(visibleSidebarColumns);
	await page.locator('[data-slot="workbench-sidebar-toggle"]').click();
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-columns')))
		.toBe(visibleSidebarColumns);
	const input = page.getByRole('textbox');
	await input.pressSequentially(
		`printf '\\344\\270\\255\\346\\226\\207-\\346\\230\\276\\347\\244\\272-OK\\n'`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibility = page.locator('[role="list"]');
	await expect(accessibility).toContainText('中文-显示-OK');
	await input.pressSequentially(
		`i=1; while [ "$i" -le 160 ]; do printf 'history-%03d searchable-agent\\n' "$i"; i=$((i+1)); done; printf 'interaction-%s-marker allowed=https://orchbee.dev blocked=file:///tmp/orchbee-blocked\\n' tail`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibilityAnnouncement = root.locator('[role="status"]').first();
	await expect(accessibility).toContainText('interaction-tail-marker');
	await expect(accessibility).toHaveAttribute('aria-live', 'off');
	await expect(accessibilityAnnouncement).toHaveAttribute('aria-live', 'polite');
	const accessibleRows = accessibility.locator(':scope > div');
	await expect
		.poll(async () => accessibleRows.count())
		.toBe(Number(await root.getAttribute('data-projected-terminal-rows')));
	expect(page.workers()).toHaveLength(0);

	const canvas = page.locator('canvas');
	const liveFirstRow = BigInt(
		(await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0',
	);
	await canvas.hover();
	await page.mouse.wheel(0, -2_000);
	await expect(accessibilityAnnouncement).toHaveAttribute('aria-live', 'off');
	await expect
		.poll(async () =>
			BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0'),
		)
		.toBeLessThan(liveFirstRow);

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await input.focus();
	await page.keyboard.press(
		process.platform === 'darwin' ? `${primaryModifier}+KeyF` : `${primaryModifier}+Shift+KeyF`,
	);
	const findSurface = page.getByRole('search');
	await expect(findSurface).toBeAttached();
	const findInput = findSurface.getByRole('textbox');
	await findInput.pressSequentially('history-001', { delay: 1 });
	await expect(findInput).toBeFocused();
	await expect(findInput).toHaveValue('history-001');
	await expect(findSurface).toHaveAttribute('data-search-state', 'complete', {
		timeout: 10_000,
	});
	await expect(findSurface.getByRole('status')).toContainText(/1.*1/u);
	await expect.poll(() => workerStarts).toBeGreaterThan(0);
	await expect.poll(() => page.workers().length).toBe(0);
	await expect(accessibility).toContainText('history-001');
	await findInput.press('Escape');
	await expect(findSurface).not.toBeAttached();

	await input.focus();
	await input.press('ArrowRight');
	await expect(accessibilityAnnouncement).toHaveAttribute('aria-live', 'polite');
	await expect(accessibility).toContainText('interaction-tail-marker');

	const markerStart = await terminalCellPoint(
		root,
		canvas,
		accessibleRows,
		'interaction-tail-marker',
		0,
		0.25,
	);
	const markerEnd = await terminalCellPoint(
		root,
		canvas,
		accessibleRows,
		'interaction-tail-marker',
		10,
		0.75,
	);
	await page.mouse.move(markerStart.x, markerStart.y);
	await page.mouse.down();
	await page.mouse.move(markerEnd.x, markerEnd.y);
	await page.mouse.up();
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-copied-bytes')))
		.toBeGreaterThan(0);
	const copyStatus = root.locator('[data-projected-terminal-status="true"][role="status"]');
	await expect(copyStatus).toBeAttached();
	await expect
		.poll(() =>
			copyStatus.evaluate((element) => {
				const style = getComputedStyle(element);
				return {
					blockSize: style.blockSize,
					clipPath: style.clipPath,
					inlineSize: style.inlineSize,
					overflow: style.overflow,
					position: style.position,
				};
			}),
		)
		.toEqual({
			blockSize: '1px',
			clipPath: 'inset(50%)',
			inlineSize: '1px',
			overflow: 'hidden',
			position: 'absolute',
		});
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toContain('interaction');

	if (process.platform === 'darwin') {
		const gestureSelectionStart = await root.getAttribute(
			'data-projected-terminal-selection-start',
		);
		await input.focus();
		await page.keyboard.press('Meta+KeyA');
		await expect
			.poll(() => root.getAttribute('data-projected-terminal-selection-start'))
			.not.toBe(gestureSelectionStart);
	}
	await input.focus();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyC' : 'Control+Shift+KeyC');
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-copied-bytes')))
		.toBeGreaterThan(0);
	await expect
		.poll(async () => application.evaluate(({ clipboard }) => clipboard.readText()))
		.toContain(process.platform === 'darwin' ? 'history-001' : 'interaction-tail');

	await application.evaluate(({ shell }) => {
		const state = globalThis as typeof globalThis & { __orchbeeOpenedTargets?: string[] };
		state.__orchbeeOpenedTargets = [];
		Object.defineProperty(shell, 'openExternal', {
			configurable: true,
			value: async (target: string) => {
				state.__orchbeeOpenedTargets?.push(target);
			},
		});
	});
	const allowedLink = await terminalCellPoint(
		root,
		canvas,
		accessibleRows,
		'https://orchbee.dev',
		4,
	);
	await page.keyboard.down(primaryModifier);
	await page.mouse.move(allowedLink.x, allowedLink.y);
	await expect.poll(() => canvas.evaluate((element) => element.style.cursor)).toBe('pointer');
	await page.mouse.click(allowedLink.x, allowedLink.y);
	await page.keyboard.up(primaryModifier);
	await expect
		.poll(() =>
			application.evaluate(
				() =>
					(globalThis as typeof globalThis & { __orchbeeOpenedTargets?: string[] })
						.__orchbeeOpenedTargets ?? [],
			),
		)
		.toEqual(['https://orchbee.dev/']);

	const blockedLink = await terminalCellPoint(
		root,
		canvas,
		accessibleRows,
		'file:///tmp/orchbee-blocked',
		4,
	);
	await page.keyboard.down(primaryModifier);
	await page.mouse.move(blockedLink.x, blockedLink.y);
	await expect.poll(() => canvas.evaluate((element) => element.style.cursor)).not.toBe('pointer');
	await page.mouse.click(blockedLink.x, blockedLink.y);
	await page.keyboard.up(primaryModifier);
	expect(
		await application.evaluate(
			() =>
				(globalThis as typeof globalThis & { __orchbeeOpenedTargets?: string[] })
					.__orchbeeOpenedTargets ?? [],
		),
	).toEqual(['https://orchbee.dev/']);

	const identityBeforeReload = await terminalDocumentIdentity(root);
	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.webContents.reload();
	});
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect
		.poll(async () => (await terminalDocumentIdentity(root)).attachmentId)
		.not.toBe(identityBeforeReload.attachmentId);
	const identityAfterReload = await terminalDocumentIdentity(root);
	expect(identityAfterReload).toMatchObject({
		creationId: identityBeforeReload.creationId,
		terminalId: identityBeforeReload.terminalId,
	});
	await expect(page.locator('[role="list"]')).toContainText('interaction-tail-marker');

	await input.focus();
	await input.pressSequentially('stty size; exit', { delay: 1 });
	await input.press('Enter');
	await expect(workbenchTabs(page)).toHaveCount(0);
	await expect(projectedTerminals(page)).toHaveCount(0);
	expect(
		await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length),
	).toBe(1);
	await expect.poll(() => traceLines.join('').includes('orchbee.desktop.copy-path')).toBe(true);
	const traceText = traceLines.join('');
	expect(traceText).toContain('"terminalFramesReceived":');
	expect(traceText).toContain('"framesForwarded":');

	await quitDesktopAutomaticallyPreservingSessions(application);
	electronApplication = undefined;
	await expect.poll(() => hostProcess?.exitCode).toBe(0);
	const directory = runtimeDirectory;
	const environment = desktopEnvironment;
	if (directory === undefined || environment === undefined) {
		throw new Error('Desktop test environment is unavailable.');
	}
	const endpointId = environment['ORCHBEE_DESKTOP_ENDPOINT_ID'];
	if (endpointId === undefined) throw new Error('Desktop endpoint identity is unavailable.');
	hostProcess = await startHost(directory, endpointId);

	const exitedRestart = await launchDesktop();
	electronApplication = exitedRestart;
	const exitedPage = await exitedRestart.firstWindow();
	await expect(exitedPage.locator('[data-slot="workbench-tab-bar"]')).toBeVisible();
	await expect(workbenchTabs(exitedPage)).toHaveCount(0);
	await expect(projectedTerminals(exitedPage)).toHaveCount(0);
	await quitDesktopAutomaticallyPreservingSessions(exitedRestart);
	electronApplication = undefined;
});

test('terminal-parity/LC-08-browser-access', async () => {
	test.setTimeout(90_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const desktopPage = await application.firstWindow();
	desktopPage.setDefaultTimeout(10_000);
	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.setSize(1_280, 720);
	});
	const desktopRoot = projectedTerminal(desktopPage);
	await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const attachmentId = await desktopRoot.getAttribute('data-projected-terminal-attachment-id');

	const settingsSurface = await openDesktopSettingsCategory(desktopPage, 'browser-access');
	const openBrowserAccess = settingsSurface.locator('main button');
	await expect(openBrowserAccess).toHaveCount(1);
	await openBrowserAccess.click();
	const dialog = desktopPage.locator('[data-slot="desktop-browser-access-dialog"]');
	await expect(dialog).toBeVisible();
	const createPersonalAccessToken = dialog.locator(
		'[data-browser-access-action="create-personal-access-token"]',
	);
	await expect
		.poll(async () => ({
			enabled: await createPersonalAccessToken.isEnabled(),
			status: await dialog.locator('[data-status]').getAttribute('data-status'),
		}))
		.toEqual({ enabled: true, status: 'missing' });
	await createPersonalAccessToken.click();
	const personalAccessTokenValue = dialog.locator(
		'[data-slot="desktop-personal-access-token-value"]',
	);
	await expect(personalAccessTokenValue).toBeVisible();
	const personalAccessToken = await personalAccessTokenValue.textContent();
	if (personalAccessToken === null || personalAccessToken.length === 0) {
		throw new Error('Desktop did not issue a Personal Access Token.');
	}
	const layers = await desktopPage.evaluate(() => {
		const dialogElement = document.querySelector('[data-slot="desktop-browser-access-dialog"]');
		const settingsElement = document.querySelector('[data-slot="terminal-settings-surface"]');
		if (dialogElement === null || settingsElement === null) {
			throw new Error('Settings and Browser access layers must both be mounted.');
		}
		return {
			dialog: Number.parseInt(getComputedStyle(dialogElement).zIndex, 10),
			settings: Number.parseInt(getComputedStyle(settingsElement).zIndex, 10),
		};
	});
	expect(layers.dialog).toBeGreaterThan(layers.settings);
	expect(await desktopRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);
	await dialog.locator('[data-browser-access-action="start-local-network"]').click();
	let origins: readonly string[] = [];
	await expect
		.poll(async () => {
			origins = await dialog
				.locator('[data-slot="desktop-browser-access-addresses"] code')
				.allTextContents();
			return origins.length;
		})
		.toBeGreaterThan(1);
	const localOrigin = origins.find((origin) => new URL(origin).hostname === '127.0.0.1');
	const networkOrigins = origins.filter((origin) => new URL(origin).hostname !== '127.0.0.1');
	expect(localOrigin).toBeDefined();
	expect(networkOrigins.length).toBeGreaterThan(0);
	expect(networkOrigins.every((origin) => isRfc1918IPv4(new URL(origin).hostname))).toBe(true);

	const networkOrigin = networkOrigins[0];
	if (networkOrigin === undefined) throw new Error('LAN Browser access origin is unavailable.');
	await dialog.locator('li').filter({ hasText: networkOrigin }).getByRole('button').click();
	let browserUrl = '';
	await expect
		.poll(async () => {
			browserUrl = await application.evaluate(({ clipboard }) => clipboard.readText());
			return browserUrl === '' ? '' : new URL(browserUrl).origin;
		})
		.toBe(networkOrigin);

	const browser = await chromium.launch();
	try {
		const browserPage = await browser.newPage();
		const browserSockets = trackGatewaySockets(browserPage);
		await signInToBrowserAccess(browserPage, browserUrl, personalAccessToken);
		expect(new URL(browserPage.url()).origin).toBe(networkOrigin);
		try {
			const browserRoot = projectedTerminal(browserPage);
			await expect(browserRoot).toHaveAttribute('data-projected-terminal-state', 'open');
			await expectTerminalSurfaceAppearance(browserRoot);
			expect(new URL(browserPage.url()).searchParams.get('orchbee-runtime')).toBe('gateway');
			await browserPage.reload();
			expect(new URL(browserPage.url()).pathname).not.toBe('/login');
			expect(new URL(browserPage.url()).searchParams.get('orchbee-runtime')).toBe('gateway');
			await expect(projectedTerminal(browserPage)).toHaveAttribute(
				'data-projected-terminal-state',
				'open',
			);

			await dialog.locator('[data-browser-access-action="reset-personal-access-token"]').click();
			const resetConfirmation = desktopPage.locator(
				'[data-slot="desktop-personal-access-token-reset-confirmation"]',
			);
			await expect(resetConfirmation).toBeVisible();
			await resetConfirmation
				.locator('[data-browser-access-action="confirm-reset-personal-access-token"]')
				.click();
			await expect(dialog.locator('[data-slot="desktop-browser-access-addresses"]')).toHaveCount(0);
			await expect(personalAccessTokenValue).toBeVisible();
			let resetPersonalAccessToken = '';
			await expect
				.poll(async () => {
					resetPersonalAccessToken = (await personalAccessTokenValue.textContent()) ?? '';
					return resetPersonalAccessToken;
				})
				.toMatch(/^[A-Za-z0-9_-]{43}$/u);
			expect(resetPersonalAccessToken).not.toBe(personalAccessToken);

			await dialog.locator('[data-browser-access-action="start-local-network"]').click();
			let resetNetworkOrigin = '';
			await expect
				.poll(async () => {
					const resetOrigins = await dialog
						.locator('[data-slot="desktop-browser-access-addresses"] code')
						.allTextContents();
					resetNetworkOrigin =
						resetOrigins.find((origin) => new URL(origin).hostname !== '127.0.0.1') ?? '';
					return resetNetworkOrigin;
				})
				.not.toBe('');
			await dialog
				.locator('li')
				.filter({ hasText: resetNetworkOrigin })
				.getByRole('button')
				.click();
			let resetBrowserUrl = '';
			await expect
				.poll(async () => {
					resetBrowserUrl = await application.evaluate(({ clipboard }) => clipboard.readText());
					return resetBrowserUrl === '' ? '' : new URL(resetBrowserUrl).origin;
				})
				.toBe(resetNetworkOrigin);

			await browserPage.goto(resetBrowserUrl);
			await expect.poll(() => new URL(browserPage.url()).pathname).toBe('/login');
			await browserPage.locator('#personal-access-token').fill(personalAccessToken);
			await browserPage.locator('[data-browser-authentication-action="submit"]').click();
			await expect(browserPage.locator('#personal-access-token')).toHaveAttribute(
				'aria-invalid',
				'true',
			);
			expect(new URL(browserPage.url()).pathname).toBe('/login');
			await browserPage.locator('#personal-access-token').fill(resetPersonalAccessToken);
			await browserPage.locator('[data-browser-authentication-action="submit"]').click();
			await expect.poll(() => new URL(browserPage.url()).pathname).not.toBe('/login');
			await expect(projectedTerminal(browserPage)).toHaveAttribute(
				'data-projected-terminal-state',
				'open',
			);
		} catch (error: unknown) {
			throw new Error(`LAN Browser access failed: ${browserSockets.events.join(' | ')}`, {
				cause: error,
			});
		}
	} finally {
		await browser.close();
	}
	await dialog.locator('[data-browser-access-action="stop"]').click();
	await expect(dialog.locator('[data-slot="desktop-browser-access-addresses"]')).toHaveCount(0);
	await desktopPage.keyboard.press('Escape');
	await expect(dialog).not.toBeVisible();
	await expect(settingsSurface).toBeVisible();
	await desktopPage.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
	expect(await desktopRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);
});

test('terminal-parity/LC-09-client-isolation', async () => {
	test.setTimeout(90_000);
	const staticApplication = electronApplication;
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	const currentRuntimeDirectory = runtimeDirectory;
	if (staticApplication === undefined) throw new Error('Electron application did not start.');
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	if (currentRuntimeDirectory === undefined) {
		throw new Error('Desktop runtime directory is unavailable.');
	}

	const staticPage = await staticApplication.firstWindow();
	const staticRoot = projectedTerminal(staticPage);
	await expect(staticRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(staticRoot).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	const staticIdentity = await terminalDocumentIdentity(staticRoot);
	await quitDesktopAutomaticallyPreservingSessions(staticApplication);
	electronApplication = undefined;

	const currentEnvironment = desktopEnvironment;
	if (currentEnvironment === undefined) throw new Error('Desktop test environment is unavailable.');
	desktopEnvironment = {
		...currentEnvironment,
		ORCHBEE_DESKTOP_RENDERER_URL: currentDevelopmentRendererUrl,
	};
	const developmentApplication = await launchDesktop();
	electronApplication = developmentApplication;
	const desktopPage = await developmentApplication.firstWindow();
	desktopPage.setDefaultTimeout(10_000);
	await developmentApplication.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.setSize(1_280, 720);
	});
	expect(new URL(desktopPage.url()).origin).toBe(new URL(currentDevelopmentRendererUrl).origin);
	expect(new URL(desktopPage.url()).searchParams.get('orchbee-runtime')).toBeNull();
	const desktopRoot = projectedTerminal(desktopPage);
	await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const desktopIdentity = await terminalDocumentIdentity(desktopRoot);
	expect(desktopIdentity).toMatchObject({
		creationId: staticIdentity.creationId,
		terminalId: staticIdentity.terminalId,
	});
	expect(desktopIdentity.attachmentId).not.toBe(staticIdentity.attachmentId);
	const desktopPresentationId = await desktopRoot.getAttribute('data-terminal-presentation-id');
	if (desktopPresentationId === null) {
		throw new Error('Desktop terminal presentation identity is unavailable.');
	}

	const browserAccess = await openLoopbackBrowserAccess(developmentApplication, desktopPage);
	const gatewayUrl = browserAccess.browserUrl;
	const browser = await chromium.launch();
	try {
		const context = await browser.newContext({ viewport: { height: 720, width: 1_280 } });
		const browserPage = await context.newPage();
		browserPage.setDefaultTimeout(10_000);
		const browserSockets = trackGatewaySockets(browserPage);
		await signInToBrowserAccess(browserPage, gatewayUrl, browserAccess.personalAccessToken);
		expect(new URL(browserPage.url()).origin).toBe(new URL(gatewayUrl).origin);
		expect(new URL(browserPage.url()).searchParams.get('orchbee-runtime')).toBe('gateway');
		await expect
			.poll(() => browserSockets.urls.some((url) => socketUsesGatewayConnection(url, gatewayUrl)))
			.toBe(true);
		const browserRoot = projectedTerminal(browserPage);
		await expect(browserRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const browserIdentity = await terminalDocumentIdentity(browserRoot);
		expect(browserIdentity).toMatchObject({
			creationId: desktopIdentity.creationId,
			terminalId: desktopIdentity.terminalId,
		});
		expect(browserIdentity.attachmentId).not.toBe(desktopIdentity.attachmentId);
		expect(await browserRoot.getAttribute('data-terminal-presentation-id')).toBe(
			desktopPresentationId,
		);

		const browserInput = browserRoot.getByRole('textbox');
		await browserInput.pressSequentially("printf 'ldw-development-marker\\n'", { delay: 1 });
		await browserInput.press('Enter');
		await expect(browserRoot.locator('[role="list"]')).toContainText('ldw-development-marker');
		await expect(desktopRoot.locator('[role="list"]')).toContainText('ldw-development-marker');

		for (const page of [desktopPage, browserPage]) {
			await expectVisibleLayout(page.locator('#root'));
			await expectVisibleLayout(page.locator('[data-slot="workbench-sidebar"]'));
			await expectVisibleLayout(page.locator('[data-slot="workbench-tab-bar"]'));
			await expectVisibleLayout(page.locator('[data-slot="workbench-panel-workspace"]'));
			await expectVisibleLayout(page.locator('[data-slot="workbench-panel"]'));
			await expectVisibleLayout(page.locator('[data-slot="workbench-terminal-viewport"]'));
		}

		await desktopPage.addInitScript(() => {
			const storageKey = 'orchbee-system-test-client-session-ids';
			window.addEventListener('message', (event: MessageEvent<unknown>) => {
				const candidate = event.data as {
					readonly bootstrap?: {
						readonly clientSession?: { readonly clientSessionId?: unknown };
					};
					readonly type?: unknown;
				};
				const clientSessionId = candidate.bootstrap?.clientSession?.clientSessionId;
				if (
					candidate.type !== 'orchbee.desktop.renderer-bootstrap-ready' ||
					typeof clientSessionId !== 'string'
				) {
					return;
				}
				const previous = JSON.parse(sessionStorage.getItem(storageKey) ?? '[]') as unknown;
				const ids = Array.isArray(previous)
					? previous.filter((value): value is string => typeof value === 'string')
					: [];
				ids.push(clientSessionId);
				sessionStorage.setItem(storageKey, JSON.stringify(ids));
			});
		});
		const desktopSocketUrls: string[] = [];
		desktopPage.on('websocket', (socket) => desktopSocketUrls.push(socket.url()));
		const identityBeforeReload = await terminalDocumentIdentity(desktopRoot);
		await developmentApplication.evaluate(({ BrowserWindow }) => {
			BrowserWindow.getAllWindows()[0]?.webContents.reload();
		});
		await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		await expect(desktopRoot.locator('[role="list"]')).toContainText('ldw-development-marker');
		await expect.poll(() => desktopClientSessionIds(desktopPage)).toHaveLength(1);
		const identityAfterFirstReload = await terminalDocumentIdentity(desktopRoot);
		expect(identityAfterFirstReload).toMatchObject({
			creationId: identityBeforeReload.creationId,
			terminalId: identityBeforeReload.terminalId,
		});
		expect(identityAfterFirstReload.attachmentId).not.toBe(identityBeforeReload.attachmentId);

		await developmentApplication.evaluate(({ BrowserWindow }) => {
			BrowserWindow.getAllWindows()[0]?.webContents.reload();
		});
		await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		await expect(desktopRoot.locator('[role="list"]')).toContainText('ldw-development-marker');
		await expect.poll(() => desktopClientSessionIds(desktopPage)).toHaveLength(2);
		const clientSessionIds = await desktopClientSessionIds(desktopPage);
		expect(new Set(clientSessionIds).size).toBe(2);
		const identityAfterSecondReload = await terminalDocumentIdentity(desktopRoot);
		expect(identityAfterSecondReload).toMatchObject({
			creationId: identityBeforeReload.creationId,
			terminalId: identityBeforeReload.terminalId,
		});
		expect(identityAfterSecondReload.attachmentId).not.toBe(identityAfterFirstReload.attachmentId);
		await expect
			.poll(() =>
				desktopSocketUrls.some((url) =>
					socketUsesRendererOrigin(url, currentDevelopmentRendererUrl),
				),
			)
			.toBe(true);

		const previewPage = await context.newPage();
		await previewPage.goto(currentDevelopmentRendererUrl);
		expect(new URL(previewPage.url()).searchParams.get('orchbee-runtime')).toBeNull();
		await expect(projectedTerminal(previewPage)).toHaveCount(0);
		await expectVisibleLayout(previewPage.locator('[data-slot="workbench-sidebar"]'));
		await expectVisibleLayout(previewPage.locator('[data-slot="workbench-tab-bar"]'));
		await expect(previewPage.locator('[data-slot="workbench-sidebar-project"]')).toHaveCount(0);

		const projectName = path.basename(currentRuntimeDirectory);
		await browserPage.getByRole('button', { name: 'Create Project' }).click();
		await browserPage.getByRole('menuitem', { name: 'Select Folder…' }).click();
		const createProjectDialog = browserPage.locator('[data-slot="workbench-project-create"]');
		await createProjectDialog.getByLabel('Folder path').fill(currentRuntimeDirectory);
		await createProjectDialog.getByRole('button', { name: 'Create' }).click();
		await expect(createProjectDialog).not.toBeAttached();
		await expandProject(browserPage, projectName);
		await expect(workbenchWorkspace(browserPage, 'Primary')).toBeVisible();
		await expandProject(desktopPage, projectName);
		await expect(workbenchWorkspace(desktopPage, 'Primary')).toBeVisible();
		await expect(previewPage.locator('[data-slot="workbench-sidebar-project"]')).toHaveCount(0);

		const attachmentClosesBeforeDisconnect = gatewaySocketEventCount(
			browserSockets,
			'socket:/v1/attachment:closed',
		);
		await browserPage.goto(currentDevelopmentRendererUrl);
		expect(new URL(browserPage.url()).searchParams.get('orchbee-runtime')).toBeNull();
		await expect(projectedTerminal(browserPage)).toHaveCount(0);
		await expect
			.poll(() => gatewaySocketEventCount(browserSockets, 'socket:/v1/attachment:closed'))
			.toBeGreaterThan(attachmentClosesBeforeDisconnect);
		await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		expect(await terminalDocumentIdentity(desktopRoot)).toMatchObject({
			creationId: identityAfterSecondReload.creationId,
			terminalId: identityAfterSecondReload.terminalId,
		});
	} finally {
		await browser.close();
	}
});

test('terminal-parity/LC-03-dual-client', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const currentRendererUrl = rendererUrl;
	if (currentRendererUrl === undefined) throw new Error('Web renderer server is unavailable.');
	const desktopPage = await application.firstWindow();
	const browserAccess = await openLoopbackBrowserAccess(application, desktopPage);
	const gatewayUrl = browserAccess.browserUrl;
	const desktopRoot = projectedTerminal(desktopPage);
	await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const desktopIdentity = await terminalDocumentIdentity(desktopRoot);
	const sharedPresentationId = await desktopRoot.getAttribute('data-terminal-presentation-id');
	const desktopInitialColumns = Number(
		await desktopRoot.getAttribute('data-projected-terminal-columns'),
	);
	if (!Number.isInteger(desktopInitialColumns) || desktopInitialColumns < 1) {
		throw new Error('Desktop terminal did not expose its initial canonical columns.');
	}

	const browser = await chromium.launch();
	try {
		const context = await browser.newContext();
		await context.grantPermissions(['clipboard-read', 'clipboard-write'], {
			origin: new URL(gatewayUrl).origin,
		});
		const firstBrowserPage = await context.newPage();
		const secondBrowserPage = await context.newPage();
		await firstBrowserPage.setViewportSize({ height: 760, width: 430 });
		const firstSockets = trackGatewaySockets(firstBrowserPage);
		const secondSockets = trackGatewaySockets(secondBrowserPage);
		await signInToBrowserAccess(firstBrowserPage, gatewayUrl, browserAccess.personalAccessToken);
		const firstRoot = projectedTerminal(firstBrowserPage);
		const secondRoot = projectedTerminal(secondBrowserPage);
		await expect(firstRoot)
			.toHaveAttribute('data-projected-terminal-state', 'open')
			.catch((error: unknown) => {
				const safeDesktopOutput = (applicationOutput.get(application) ?? [])
					.join('')
					.split(/\r?\n/u);
				throw new Error(
					`First Browser projection failed. Browser=${JSON.stringify(firstSockets.events)} Desktop=${safeDesktopOutput.join('\n')}`,
					{ cause: error },
				);
			});
		const firstInput = firstBrowserPage.getByRole('textbox');
		await firstBrowserPage.evaluate(() => {
			if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
		});
		await firstInput.focus();
		await expect
			.poll(async () => Number(await firstRoot.getAttribute('data-projected-terminal-columns')))
			.toBeLessThan(desktopInitialColumns)
			.catch(async (error: unknown) => {
				const metrics = await firstRoot.evaluate((element) => {
					const bounds = element.getBoundingClientRect();
					const canvasBounds = element.querySelector('canvas')?.getBoundingClientRect();
					return {
						canvasHeight: canvasBounds?.height ?? null,
						canvasWidth: canvasBounds?.width ?? null,
						columns: element.dataset['projectedTerminalColumns'] ?? null,
						rootHeight: bounds.height,
						rootWidth: bounds.width,
						viewportHeight: window.innerHeight,
						viewportWidth: window.innerWidth,
					};
				});
				throw new Error(
					`Mobile resize ownership did not converge: ${JSON.stringify({ metrics, socketEvents: firstSockets.events })}`,
					{ cause: error },
				);
			});
		const mobileColumns = Number(await firstRoot.getAttribute('data-projected-terminal-columns'));
		await expect
			.poll(async () => Number(await desktopRoot.getAttribute('data-projected-terminal-columns')))
			.toBe(mobileColumns);
		await secondBrowserPage.goto(gatewayUrl);
		await expect(secondRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const firstIdentity = await terminalDocumentIdentity(firstRoot);
		const secondIdentity = await terminalDocumentIdentity(secondRoot);
		expect(firstIdentity).toMatchObject({
			creationId: desktopIdentity.creationId,
			terminalId: desktopIdentity.terminalId,
		});
		expect(secondIdentity).toMatchObject({
			creationId: desktopIdentity.creationId,
			terminalId: desktopIdentity.terminalId,
		});
		expect(
			new Set([
				desktopIdentity.attachmentId,
				firstIdentity.attachmentId,
				secondIdentity.attachmentId,
			]).size,
		).toBe(3);
		expect(await firstRoot.getAttribute('data-terminal-presentation-id')).toBe(
			sharedPresentationId,
		);
		expect(await secondRoot.getAttribute('data-terminal-presentation-id')).toBe(
			sharedPresentationId,
		);
		await expect.poll(() => gatewaySocketCount(firstSockets, '/v1/control')).toBe(1);
		await expect.poll(() => gatewaySocketCount(secondSockets, '/v1/control')).toBe(1);
		await expect.poll(() => gatewaySocketCount(firstSockets, '/v1/attachment')).toBe(1);
		await expect.poll(() => gatewaySocketCount(secondSockets, '/v1/attachment')).toBe(1);

		await firstInput.pressSequentially("printf 'gateway-shared-terminal-marker\\n'");
		await firstInput.press('Enter');
		await expect(firstBrowserPage.locator('[role="list"]')).toContainText(
			'gateway-shared-terminal-marker',
		);
		await expect(secondBrowserPage.locator('[role="list"]')).toContainText(
			'gateway-shared-terminal-marker',
		);
		await expect(desktopPage.locator('[role="list"]')).toContainText(
			'gateway-shared-terminal-marker',
		);

		await firstBrowserPage.getByRole('button', { name: 'New tab' }).click();
		const firstTabs = workbenchTabs(firstBrowserPage);
		const secondTabs = workbenchTabs(secondBrowserPage);
		const desktopTabs = workbenchTabs(desktopPage);
		await expect(firstTabs).toHaveCount(2);
		await expect(secondTabs).toHaveCount(2);
		await expect(desktopTabs).toHaveCount(2);
		await expect(projectedTerminal(firstBrowserPage)).toHaveAttribute(
			'data-projected-terminal-state',
			'open',
		);
		expect(
			await projectedTerminal(firstBrowserPage).getAttribute('data-terminal-presentation-id'),
		).not.toBe(sharedPresentationId);
		expect(
			await projectedTerminal(secondBrowserPage).getAttribute('data-terminal-presentation-id'),
		).toBe(sharedPresentationId);
		expect(await projectedTerminal(desktopPage).getAttribute('data-terminal-presentation-id')).toBe(
			sharedPresentationId,
		);

		await firstBrowserPage.close();
		const secondInput = secondBrowserPage.getByRole('textbox');
		await secondInput.pressSequentially("printf 'gateway-after-disconnect-marker\\n'");
		await secondInput.press('Enter');
		await expect(desktopPage.locator('[role="list"]')).toContainText(
			'gateway-after-disconnect-marker',
		);

		const replacementPage = await context.newPage();
		const replacementSockets = trackGatewaySockets(replacementPage);
		await replacementPage.goto(gatewayUrl);
		await expect(workbenchTabs(replacementPage)).toHaveCount(2);
		await workbenchTabs(replacementPage).first().click();
		const replacementRoot = projectedTerminal(replacementPage);
		await expect(replacementRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		expect(await replacementRoot.getAttribute('data-terminal-presentation-id')).toBe(
			sharedPresentationId,
		);
		const replacementIdentity = await terminalDocumentIdentity(replacementRoot);
		expect(replacementIdentity).toMatchObject({
			creationId: desktopIdentity.creationId,
			terminalId: desktopIdentity.terminalId,
		});
		expect(replacementIdentity.attachmentId).not.toBe(firstIdentity.attachmentId);
		await expect(replacementPage.locator('[role="list"]')).toContainText(
			'gateway-after-disconnect-marker',
		);
		await expect.poll(() => gatewaySocketCount(replacementSockets, '/v1/control')).toBe(1);
		await expect
			.poll(() => gatewaySocketCount(replacementSockets, '/v1/attachment'))
			.toBeGreaterThanOrEqual(1);

		const linkTarget = new URL('/gateway-link-target', currentRendererUrl).href;
		await secondInput.pressSequentially(`printf 'browser-copy-marker ${linkTarget}\\n'`);
		await secondInput.press('Enter');
		const secondLog = secondBrowserPage.locator('[role="list"]');
		await expect(secondLog).toContainText('browser-copy-marker');
		await waitForTerminalPresentationStability(secondRoot);
		const secondCanvas = secondBrowserPage.locator('canvas');
		const secondRows = secondLog.locator(':scope > div');
		const selectionStart = await terminalCellPoint(
			secondRoot,
			secondCanvas,
			secondRows,
			'browser-copy-marker',
			0,
			0.25,
		);
		const selectionEnd = await terminalCellPoint(
			secondRoot,
			secondCanvas,
			secondRows,
			'browser-copy-marker',
			18,
			0.75,
		);
		await secondBrowserPage.mouse.move(selectionStart.x, selectionStart.y);
		await secondBrowserPage.mouse.down();
		await secondBrowserPage.mouse.move(selectionEnd.x, selectionEnd.y);
		await secondBrowserPage.mouse.up();
		await expect(secondRoot).toHaveAttribute('data-projected-terminal-selection-state', 'active');
		await secondInput.focus();
		await secondBrowserPage.keyboard.press(
			process.platform === 'darwin' ? 'Meta+KeyC' : 'Control+Shift+KeyC',
		);
		await expect
			.poll(async () =>
				Number(await secondRoot.getAttribute('data-projected-terminal-copied-bytes')),
			)
			.toBeGreaterThan(0);
		await expect
			.poll(() => secondBrowserPage.evaluate(() => navigator.clipboard.readText()))
			.toContain('browser-copy-marker');

		const link = await terminalCellPoint(secondRoot, secondCanvas, secondRows, linkTarget, 4);
		const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
		const popupPromise = secondBrowserPage.waitForEvent('popup');
		await secondBrowserPage.keyboard.down(primaryModifier);
		await secondBrowserPage.mouse.move(link.x, link.y);
		await expect
			.poll(() => secondCanvas.evaluate((element) => element.style.cursor))
			.toBe('pointer');
		await secondBrowserPage.mouse.click(link.x, link.y);
		await secondBrowserPage.keyboard.up(primaryModifier);
		const popup = await popupPromise;
		await popup.waitForLoadState('domcontentloaded');
		expect(new URL(popup.url()).pathname).toBe('/gateway-link-target');
		await popup.close();

		const unauthenticatedContext = await browser.newContext();
		try {
			const rejectedPage = await unauthenticatedContext.newPage();
			const rejectedSockets = trackGatewaySockets(rejectedPage);
			await rejectedPage.goto(gatewayUrl);
			expect(new URL(rejectedPage.url()).pathname).toBe('/login');
			await expect(projectedTerminal(rejectedPage)).toHaveCount(0);
			expect(gatewaySocketCount(rejectedSockets, '/v1/control')).toBe(0);
		} finally {
			await unauthenticatedContext.close();
		}
	} finally {
		await browser.close();
	}
});

test('runs the complete shared Project, Workspace, Tab, Panel, and terminal flow', async () => {
	test.setTimeout(90_000);
	let application = electronApplication;
	const currentRuntimeDirectory = runtimeDirectory;
	if (application === undefined) throw new Error('Electron application did not start.');
	if (currentRuntimeDirectory === undefined) {
		throw new Error('Desktop runtime directory is unavailable.');
	}
	const projectName = path.basename(currentRuntimeDirectory);
	let desktopPage = await application.firstWindow();
	const browserAccess = await openLoopbackBrowserAccess(application, desktopPage);
	const gatewayUrl = browserAccess.browserUrl;
	desktopPage.setDefaultTimeout(10_000);
	const browser = await chromium.launch();
	let browserContext = await browser.newContext();
	let browserPage = await browserContext.newPage();
	browserPage.setDefaultTimeout(10_000);
	const browserSockets = trackGatewaySockets(browserPage);

	try {
		await signInToBrowserAccess(browserPage, gatewayUrl, browserAccess.personalAccessToken);
		await expect(projectedTerminal(browserPage)).toHaveAttribute(
			'data-projected-terminal-state',
			'open',
		);

		await browserPage.getByRole('button', { name: 'Create Project' }).click();
		await browserPage.getByRole('menuitem', { name: 'Select Folder…' }).click();
		const createProjectDialog = browserPage.locator('[data-slot="workbench-project-create"]');
		await createProjectDialog.getByLabel('Folder path').fill(currentRuntimeDirectory);
		await createProjectDialog.getByRole('button', { name: 'Create' }).click();
		await expect(createProjectDialog).not.toBeAttached();
		await expandProject(browserPage, projectName);
		await expect(workbenchWorkspace(browserPage, 'Primary')).toBeVisible();

		const browserProjectRoot = projectedTerminal(browserPage);
		await expect(browserProjectRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const primaryPresentationId = await browserProjectRoot.getAttribute(
			'data-terminal-presentation-id',
		);
		if (primaryPresentationId === null) {
			throw new Error('Primary Workspace presentation identity is unavailable.');
		}
		const primaryInput = browserProjectRoot.getByRole('textbox');
		await expect(primaryInput).toBeFocused();
		await primaryInput.pressSequentially("pwd; printf 'lwf-primary-marker\\n'", { delay: 1 });
		await primaryInput.press('Enter');
		await expect(browserProjectRoot.locator('[role="list"]')).toContainText(
			currentRuntimeDirectory,
		);
		await expect(browserProjectRoot.locator('[role="list"]')).toContainText('lwf-primary-marker');

		await expandProject(desktopPage, projectName);
		await workbenchWorkspace(desktopPage, 'Primary').click();
		const desktopPrimaryRoot = projectedTerminalFor(desktopPage, primaryPresentationId);
		await expect(desktopPrimaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		await expect(desktopPrimaryRoot.locator('[role="list"]')).toContainText('lwf-primary-marker');
		const primaryIdentity = await terminalDocumentIdentity(browserProjectRoot);
		expect(await terminalDocumentIdentity(desktopPrimaryRoot)).toMatchObject({
			creationId: primaryIdentity.creationId,
			terminalId: primaryIdentity.terminalId,
		});

		await browserPage
			.locator(
				`[data-slot="workbench-terminal-viewport"][data-terminal-presentation-id="${primaryPresentationId}"]`,
			)
			.click({ button: 'right' });
		await browserPage.getByRole('menuitem', { name: 'Split right' }).click();
		await expect(projectedTerminals(browserPage)).toHaveCount(2);
		await expect(projectedTerminals(desktopPage)).toHaveCount(2);
		await waitForProjectedTerminalsOpen(browserPage);
		await waitForProjectedTerminalsOpen(desktopPage);
		const splitPresentationIds = await projectedTerminalPresentationIds(browserPage);
		const splitPresentationId = splitPresentationIds.find(
			(presentationId) => presentationId !== primaryPresentationId,
		);
		if (splitPresentationId === undefined) {
			throw new Error('Split terminal presentation identity is unavailable.');
		}
		const splitRoot = projectedTerminalFor(browserPage, splitPresentationId);
		const splitInput = splitRoot.getByRole('textbox');
		await expect(splitInput).toBeFocused();
		await splitInput.pressSequentially("printf 'lwf-split-marker\\n'; sleep 300", { delay: 1 });
		await splitInput.press('Enter');
		await expect(
			projectedTerminalFor(desktopPage, splitPresentationId).locator('[role="list"]'),
		).toContainText('lwf-split-marker');
		const splitIdentity = await terminalDocumentIdentity(splitRoot);
		await expect(browserProjectRoot).toHaveAttribute(
			'data-projected-terminal-rendered-revision',
			/^\d+$/u,
		);
		await expect(splitRoot).toHaveAttribute('data-projected-terminal-rendered-revision', /^\d+$/u);
		const tabHandoffTrace = await browserPage.evaluateHandle(() => {
			const entries: Array<{
				readonly blankTop: boolean;
				readonly modes: readonly string[];
			}> = [];
			const control = { animationFrame: 0, running: true };
			const record = (): void => {
				const workspace = document.querySelector('[data-slot="workbench-panel-workspace"]');
				if (!(workspace instanceof HTMLElement)) {
					entries.push({ blankTop: true, modes: [] });
					return;
				}
				const layers = Array.from(workspace.children).filter(
					(element): element is HTMLElement =>
						element instanceof HTMLElement && element.dataset['presentationMode'] !== undefined,
				);
				const top =
					layers.find((layer) => layer.dataset['presentationMode'] === 'retained') ??
					layers.find((layer) => layer.dataset['presentationMode'] === 'incoming') ??
					layers.find((layer) => layer.dataset['presentationMode'] === 'active');
				const terminalRoots =
					top === undefined
						? []
						: Array.from(top.querySelectorAll('[data-terminal-projection-state]')).filter(
								(element): element is HTMLElement => element instanceof HTMLElement,
							);
				const meaningful =
					terminalRoots.length > 0 &&
					terminalRoots.every(
						(root) =>
							root.dataset['projectedTerminalCachedPresentation'] === 'true' ||
							root.dataset['projectedTerminalRenderedRevision'] !== undefined,
					);
				entries.push({
					blankTop: !meaningful,
					modes: layers.flatMap((layer) => layer.dataset['presentationMode'] ?? []),
				});
			};
			const observer = new MutationObserver(record);
			observer.observe(document.body, {
				attributeFilter: [
					'data-presentation-mode',
					'data-projected-terminal-cached-presentation',
					'data-projected-terminal-rendered-revision',
				],
				attributes: true,
				childList: true,
				subtree: true,
			});
			const sampleFrame = (): void => {
				if (!control.running) return;
				record();
				control.animationFrame = requestAnimationFrame(sampleFrame);
			};
			record();
			control.animationFrame = requestAnimationFrame(sampleFrame);
			return { control, entries, observer };
		});

		const attachmentClosesBeforeTabSwitch = gatewaySocketEventCount(
			browserSockets,
			'socket:/v1/attachment:closed',
		);
		await browserPage.getByRole('button', { name: 'New tab' }).click();
		await expect(workbenchTabs(browserPage)).toHaveCount(2);
		await expect(workbenchTabs(desktopPage)).toHaveCount(2);
		await expect(projectedTerminals(browserPage)).toHaveCount(1);
		await expect(projectedTerminals(desktopPage)).toHaveCount(2);
		await expect
			.poll(() => gatewaySocketEventCount(browserSockets, 'socket:/v1/attachment:closed'))
			.toBeGreaterThanOrEqual(attachmentClosesBeforeTabSwitch + 2);
		const secondTabRoot = projectedTerminal(browserPage);
		await expect(secondTabRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		await expect(secondTabRoot.getByRole('textbox')).toBeFocused();
		const secondTabPresentationId = await secondTabRoot.getAttribute(
			'data-terminal-presentation-id',
		);
		if (secondTabPresentationId === null) {
			throw new Error('Second Tab presentation identity is unavailable.');
		}
		const secondTabIdentity = await terminalDocumentIdentity(secondTabRoot);
		expect(secondTabIdentity.terminalId).not.toBe(primaryIdentity.terminalId);
		expect(secondTabIdentity.terminalId).not.toBe(splitIdentity.terminalId);
		expect(await projectedTerminal(desktopPage).getAttribute('data-terminal-presentation-id')).toBe(
			primaryPresentationId,
		);

		await workbenchTabs(browserPage).first().click();
		await expect(projectedTerminals(browserPage)).toHaveCount(2);
		await waitForProjectedTerminalsOpen(browserPage);
		const primaryIdentityAfterTabReturn = await terminalDocumentIdentity(
			projectedTerminalFor(browserPage, primaryPresentationId),
		);
		const splitIdentityAfterTabReturn = await terminalDocumentIdentity(
			projectedTerminalFor(browserPage, splitPresentationId),
		);
		expect(primaryIdentityAfterTabReturn).toMatchObject({
			creationId: primaryIdentity.creationId,
			terminalId: primaryIdentity.terminalId,
		});
		expect(splitIdentityAfterTabReturn).toMatchObject({
			creationId: splitIdentity.creationId,
			terminalId: splitIdentity.terminalId,
		});
		await expect(
			projectedTerminalFor(browserPage, splitPresentationId).getByRole('textbox'),
		).toBeFocused();
		await expect(projectedTerminalFor(browserPage, splitPresentationId)).toHaveAttribute(
			'data-projected-terminal-close-activity',
			'active',
		);
		expect(primaryIdentityAfterTabReturn.attachmentId).not.toBe(primaryIdentity.attachmentId);
		expect(splitIdentityAfterTabReturn.attachmentId).not.toBe(splitIdentity.attachmentId);
		const handoffEntries = await tabHandoffTrace.evaluate((trace) => {
			trace.control.running = false;
			cancelAnimationFrame(trace.control.animationFrame);
			trace.observer.disconnect();
			return trace.entries;
		});
		await tabHandoffTrace.dispose();
		expect(handoffEntries.length).toBeGreaterThan(1);
		expect(
			handoffEntries.some(
				(entry) => entry.modes.includes('incoming') && entry.modes.includes('retained'),
			),
		).toBe(true);
		expect(handoffEntries.filter((entry) => entry.blankTop)).toEqual([]);

		const splitPanel = browserPage.locator('[data-slot="workbench-panel"]').filter({
			has: projectedTerminalFor(browserPage, splitPresentationId),
		});
		await splitPanel.getByRole('button', { name: 'Close' }).click();
		const closeConfirmation = browserPage.getByRole('alertdialog', {
			name: 'Close running terminal?',
		});
		await expect(closeConfirmation).toBeVisible();
		await closeConfirmation.getByRole('button', { name: 'Close anyway' }).click();
		await expect(projectedTerminalFor(browserPage, splitPresentationId)).toHaveCount(0);
		await expect(projectedTerminalFor(desktopPage, splitPresentationId)).toHaveCount(0);
		await expect(projectedTerminals(browserPage)).toHaveCount(1);
		await expect(projectedTerminals(desktopPage)).toHaveCount(1);
		await expect(projectedTerminal(browserPage).getByRole('textbox')).toBeFocused();
		const retainedPrimaryIdentity = await terminalDocumentIdentity(projectedTerminal(browserPage));
		expect(retainedPrimaryIdentity).toMatchObject({
			creationId: primaryIdentity.creationId,
			terminalId: primaryIdentity.terminalId,
		});

		const browserProject = browserPage.locator('li[data-project-id]').filter({
			has: browserPage.getByRole('button', { name: `Collapse project: ${projectName}` }),
		});
		await browserProject
			.getByRole('button', { name: `Collapse project: ${projectName}` })
			.click({ button: 'right' });
		await browserPage.getByRole('menuitem', { name: 'New Workspace' }).hover();
		const projectFolderMenuItem = browserPage.getByRole('menuitem', {
			exact: true,
			name: projectName,
		});
		await expect(projectFolderMenuItem).toBeVisible();
		await projectFolderMenuItem.click();
		await expect(workbenchWorkspace(browserPage, projectName)).toBeVisible();
		await expect(workbenchWorkspace(desktopPage, projectName)).toBeVisible();
		await expect(projectedTerminals(browserPage)).toHaveCount(1);
		await expect(projectedTerminals(desktopPage)).toHaveCount(1);
		const secondaryWorkspaceRoot = projectedTerminal(browserPage);
		await expect(secondaryWorkspaceRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const secondaryPresentationId = await secondaryWorkspaceRoot.getAttribute(
			'data-terminal-presentation-id',
		);
		if (secondaryPresentationId === null) {
			throw new Error('Secondary Workspace presentation identity is unavailable.');
		}
		const secondaryInput = secondaryWorkspaceRoot.getByRole('textbox');
		await secondaryInput.pressSequentially("pwd; printf 'lwf-secondary-marker\\n'", { delay: 1 });
		await secondaryInput.press('Enter');
		await expect(secondaryWorkspaceRoot.locator('[role="list"]')).toContainText(
			currentRuntimeDirectory,
		);
		const secondaryIdentity = await terminalDocumentIdentity(secondaryWorkspaceRoot);
		expect(secondaryIdentity.terminalId).not.toBe(primaryIdentity.terminalId);
		expect(await projectedTerminal(desktopPage).getAttribute('data-terminal-presentation-id')).toBe(
			primaryPresentationId,
		);
		if (secondTabIdentity.terminalId === null) {
			throw new Error('Second Tab Host terminal identity is unavailable.');
		}
		const bindingStorePath = path.join(currentRuntimeDirectory, 'bindings-v3.json');
		expect(await readFile(bindingStorePath, 'utf8')).toContain(secondTabIdentity.terminalId);

		await browserContext.close();
		await quitDesktopPreservingSessions(application, desktopPage);
		electronApplication = undefined;
		expect(await readFile(bindingStorePath, 'utf8')).toContain(secondTabIdentity.terminalId);
		application = await launchDesktop();
		electronApplication = application;
		desktopPage = await application.firstWindow();
		desktopPage.setDefaultTimeout(10_000);
		expect(await readFile(bindingStorePath, 'utf8')).toContain(secondTabIdentity.terminalId);
		await expandProject(desktopPage, projectName);
		await expect(desktopPage.locator('[data-slot="workbench-sidebar-workspace"]')).toHaveCount(2);
		await workbenchWorkspace(desktopPage, 'Primary').click();
		await expect(workbenchTabs(desktopPage)).toHaveCount(2);
		await workbenchTabs(desktopPage).first().click();
		const restartedPrimaryRoot = projectedTerminalFor(desktopPage, primaryPresentationId);
		await expect(restartedPrimaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		expect(await terminalDocumentIdentity(restartedPrimaryRoot)).toMatchObject({
			creationId: primaryIdentity.creationId,
			terminalId: primaryIdentity.terminalId,
		});
		await workbenchTabs(desktopPage).nth(1).click();
		const restartedSecondTabRoot = projectedTerminalFor(desktopPage, secondTabPresentationId);
		await expect(restartedSecondTabRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		expect(await terminalDocumentIdentity(restartedSecondTabRoot)).toMatchObject({
			creationId: secondTabIdentity.creationId,
			terminalId: secondTabIdentity.terminalId,
		});
		await workbenchWorkspace(desktopPage, projectName).click();
		const restartedSecondaryRoot = projectedTerminalFor(desktopPage, secondaryPresentationId);
		await expect(restartedSecondaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		expect(await terminalDocumentIdentity(restartedSecondaryRoot)).toMatchObject({
			creationId: secondaryIdentity.creationId,
			terminalId: secondaryIdentity.terminalId,
		});

		const restartedBrowserAccess = await openLoopbackBrowserAccess(
			application,
			desktopPage,
			browserAccess.personalAccessToken,
		);
		const restartedGatewayUrl = restartedBrowserAccess.browserUrl;
		browserContext = await browser.newContext();
		browserPage = await browserContext.newPage();
		browserPage.setDefaultTimeout(10_000);
		await signInToBrowserAccess(
			browserPage,
			restartedGatewayUrl,
			restartedBrowserAccess.personalAccessToken,
		);
		await expandProject(browserPage, projectName);
		await expect(browserPage.locator('[data-slot="workbench-sidebar-workspace"]')).toHaveCount(2);
		await workbenchWorkspace(browserPage, 'Primary').click();
		await expect(workbenchTabs(browserPage)).toHaveCount(2);
		await workbenchTabs(browserPage).first().click();
		const reconnectedPrimaryRoot = projectedTerminalFor(browserPage, primaryPresentationId);
		await expect(reconnectedPrimaryRoot).toHaveAttribute('data-projected-terminal-state', 'open');
		const reconnectedPrimaryIdentity = await terminalDocumentIdentity(reconnectedPrimaryRoot);
		expect(reconnectedPrimaryIdentity).toMatchObject({
			creationId: primaryIdentity.creationId,
			terminalId: primaryIdentity.terminalId,
		});
		expect(reconnectedPrimaryIdentity.attachmentId).not.toBe(
			primaryIdentityAfterTabReturn.attachmentId,
		);
	} finally {
		await browser.close();
	}
});

test('restarts Electron Main while retaining the Host terminal identity and contents', async () => {
	const firstApplication = electronApplication;
	if (firstApplication === undefined) throw new Error('Electron application did not start.');
	const firstPage = await firstApplication.firstWindow();
	const firstRoot = projectedTerminal(firstPage);
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = firstPage.getByRole('textbox');
	await input.pressSequentially("printf 'recovery-main-marker\\n'");
	await input.press('Enter');
	await expect(firstPage.locator('[role="list"]')).toContainText('recovery-main-marker');
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	const identityBeforeRestart = await terminalDocumentIdentity(firstRoot);
	if (runtimeDirectory === undefined) throw new Error('Desktop runtime directory is unavailable.');
	expect(
		JSON.parse(await readFile(path.join(runtimeDirectory, 'bindings-v3.json'), 'utf8')),
	).toMatchObject({
		bindings: [
			{
				creationId: identityBeforeRestart.creationId,
				presentationId: expect.any(String),
				state: { kind: 'retained' },
			},
		],
		version: 3,
	});

	await quitDesktopAutomaticallyPreservingSessions(firstApplication);
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();

	const restartedApplication = await launchDesktop();
	electronApplication = restartedApplication;
	const restartedPage = await restartedApplication.firstWindow();
	const restartedRoot = projectedTerminal(restartedPage);
	await expect(restartedRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(restartedPage.locator('[role="list"]')).toContainText('recovery-main-marker');
	const identityAfterRestart = await terminalDocumentIdentity(restartedRoot);
	expect(identityAfterRestart).toMatchObject({
		creationId: identityBeforeRestart.creationId,
		terminalId: identityBeforeRestart.terminalId,
	});
	expect(identityAfterRestart.attachmentId).not.toBe(identityBeforeRestart.attachmentId);
	expect(identityAfterRestart.hostEpoch).toBe(identityBeforeRestart.hostEpoch);
});

test('stops Host sessions while retaining topology and starts a fresh session explicitly', async () => {
	const firstApplication = electronApplication;
	if (firstApplication === undefined) throw new Error('Electron application did not start.');
	const firstPage = await firstApplication.firstWindow();
	const firstRoot = projectedTerminal(firstPage);
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const stoppedPresentationId = await firstRoot.getAttribute('data-terminal-presentation-id');
	const input = firstPage.getByRole('textbox');
	await input.pressSequentially("printf 'stop-policy-marker\\n'; sleep 300");
	await input.press('Enter');
	await expect(firstPage.locator('[role="list"]')).toContainText('stop-policy-marker');
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-close-activity', 'active');

	await quitDesktopWithChoice(firstApplication, firstPage, 'stop');
	electronApplication = undefined;
	await expect.poll(() => hostProcess?.exitCode).toBe(0);
	if (runtimeDirectory === undefined || desktopEnvironment === undefined) {
		throw new Error('Desktop test environment is unavailable.');
	}
	const endpointId = desktopEnvironment['ORCHBEE_DESKTOP_ENDPOINT_ID'];
	if (endpointId === undefined) throw new Error('Desktop endpoint identity is unavailable.');
	hostProcess = await startHost(runtimeDirectory, endpointId);

	const restartedApplication = await launchDesktop();
	electronApplication = restartedApplication;
	const restartedPage = await restartedApplication.firstWindow();
	const stoppedSurface = restartedPage.locator('[data-terminal-presentation-id]').first();
	await expect(stoppedSurface.locator('[data-slot="projected-terminal-stopped"]')).toBeVisible();
	expect(await stoppedSurface.getAttribute('data-terminal-presentation-id')).toBe(
		stoppedPresentationId,
	);
	const stoppedProjection = stoppedSurface.locator('[data-terminal-projection-state]');
	await expect(stoppedProjection).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(restartedPage.locator('[role="list"]')).toContainText('stop-policy-marker');
	await stoppedSurface.locator('[data-slot="projected-terminal-start-new-session"]').click();
	const restartedRoot = projectedTerminal(restartedPage);
	await expect(restartedRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(restartedRoot).toHaveAttribute('data-projected-terminal-close-activity', 'idle');
	await expect(restartedPage.locator('[role="list"]')).not.toContainText('stop-policy-marker');
	expect(await restartedRoot.getAttribute('data-terminal-presentation-id')).not.toBe(
		stoppedPresentationId,
	);

	await quitDesktopAutomaticallyPreservingSessions(restartedApplication);
	electronApplication = undefined;
});

async function launchDesktop(): Promise<ElectronApplication> {
	if (desktopEnvironment === undefined) throw new Error('Desktop test environment is unavailable.');
	if (runtimeDirectory === undefined) throw new Error('Desktop runtime directory is unavailable.');
	if (desktopMainEntry === undefined) throw new Error('Desktop test Main entry is unavailable.');
	const application = await electron.launch({
		args: [
			`--user-data-dir=${path.join(runtimeDirectory, 'electron-user-data')}`,
			desktopMainEntry,
		],
		env: desktopEnvironment,
	});
	const output: string[] = [];
	application.process().stdout?.on('data', (chunk: Buffer) => output.push(chunk.toString()));
	application.process().stderr?.on('data', (chunk: Buffer) => output.push(chunk.toString()));
	applicationOutput.set(application, output);
	return application;
}

async function invokeDesktopWindowMenuItem(
	application: ElectronApplication,
	menuItemId: string,
	triggeredByAccelerator: boolean,
): Promise<void> {
	await expect
		.poll(() =>
			application.evaluate(
				({ Menu }, id) => Boolean(Menu.getApplicationMenu()?.getMenuItemById(id)),
				menuItemId,
			),
		)
		.toBe(true);
	await application.evaluate(
		({ BrowserWindow, Menu }, input) => {
			const window = BrowserWindow.getAllWindows()[0];
			const item = Menu.getApplicationMenu()?.getMenuItemById(input.menuItemId);
			if (window === undefined || item === null || item === undefined) {
				throw new Error(`Desktop menu item ${input.menuItemId} is unavailable.`);
			}
			item.click(
				{ triggeredByAccelerator: input.triggeredByAccelerator },
				window,
				window.webContents,
			);
		},
		{ menuItemId, triggeredByAccelerator },
	);
}

async function openLoopbackBrowserAccess(
	application: ElectronApplication,
	page: Page,
	existingPersonalAccessToken?: string,
): Promise<{ readonly browserUrl: string; readonly personalAccessToken: string }> {
	const terminalRoot = projectedTerminal(page);
	const attachmentId = await terminalRoot.getAttribute('data-projected-terminal-attachment-id');
	await application.evaluate(({ shell }) => {
		const state = globalThis as typeof globalThis & { __orchbeeBrowserAccessUrl?: string };
		delete state.__orchbeeBrowserAccessUrl;
		Object.defineProperty(shell, 'openExternal', {
			configurable: true,
			value: async (url: string) => {
				state.__orchbeeBrowserAccessUrl = url;
			},
		});
	});
	const settingsSurface = await openDesktopSettingsCategory(page, 'browser-access');
	const openBrowserAccess = settingsSurface.locator('main button');
	await expect(openBrowserAccess).toHaveCount(1);
	await openBrowserAccess.click();
	const dialog = page.locator('[data-slot="desktop-browser-access-dialog"]');
	await expect(dialog).toBeVisible();
	const personalAccessTokenAction = dialog.locator(
		'[data-browser-access-action="create-personal-access-token"], [data-browser-access-action="reset-personal-access-token"]',
	);
	await expect(personalAccessTokenAction).toBeEnabled();
	let personalAccessToken = existingPersonalAccessToken;
	if (
		(await personalAccessTokenAction.getAttribute('data-browser-access-action')) ===
		'create-personal-access-token'
	) {
		await personalAccessTokenAction.click();
		const tokenValue = dialog.locator('[data-slot="desktop-personal-access-token-value"]');
		await expect(tokenValue).toBeVisible();
		personalAccessToken = (await tokenValue.textContent()) ?? undefined;
	}
	if (personalAccessToken === undefined || personalAccessToken.length === 0) {
		throw new Error('Desktop did not expose a new Personal Access Token.');
	}
	await dialog.locator('[data-browser-access-action="open-local"]').click();
	let browserUrl: string | null = null;
	await expect
		.poll(async () => {
			browserUrl = await application.evaluate(
				() =>
					(globalThis as typeof globalThis & { __orchbeeBrowserAccessUrl?: string })
						.__orchbeeBrowserAccessUrl ?? null,
			);
			return browserUrl;
		})
		.not.toBeNull();
	await page.keyboard.press('Escape');
	await expect(dialog).not.toBeVisible();
	await expect(settingsSurface).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(settingsSurface).not.toBeVisible();
	expect(await terminalRoot.getAttribute('data-projected-terminal-attachment-id')).toBe(
		attachmentId,
	);
	if (browserUrl === null) throw new Error('Desktop did not open a Browser access URL.');
	return { browserUrl, personalAccessToken };
}

async function signInToBrowserAccess(
	page: Page,
	browserUrl: string,
	personalAccessToken: string,
): Promise<void> {
	await page.goto(browserUrl);
	await expect.poll(() => new URL(page.url()).pathname).toBe('/login');
	await page.locator('#personal-access-token').fill(personalAccessToken);
	await page.locator('[data-browser-authentication-action="submit"]').click();
	await expect.poll(() => new URL(page.url()).pathname).not.toBe('/login');
	expect(new URL(page.url()).searchParams.get('orchbee-runtime')).toBe('gateway');
}

async function openDesktopSettingsCategory(
	page: Page,
	category:
		| 'browser-access'
		| 'keyboard-shortcuts'
		| 'profiles'
		| 'remote-development'
		| 'shell-integration',
): Promise<Locator> {
	await page.locator('[data-slot="workbench-sidebar-settings"]').click();
	const settingsSurface = page.locator('[data-slot="terminal-settings-surface"]');
	await expect(settingsSurface).toBeVisible();
	const categoryButton = settingsSurface.locator(
		`nav button[data-settings-category="${category}"]`,
	);
	await expect(categoryButton).toBeVisible();
	await categoryButton.click();
	await expect(categoryButton).toHaveAttribute('aria-current', 'page');
	return settingsSurface;
}

async function startLoopbackSshFixture(directory: string): Promise<{
	readonly clientKey: string;
	readonly port: number;
	readonly server: ChildProcessWithoutNullStreams;
	readonly username: string;
}> {
	const fixtureDirectory = path.join(directory, 'loopback-sshd');
	await mkdir(fixtureDirectory, { mode: 0o700 });
	const hostKey = path.join(fixtureDirectory, 'host-key');
	const clientKey = path.join(fixtureDirectory, 'client-key');
	await runProgram('/usr/bin/ssh-keygen', ['-q', '-t', 'ed25519', '-N', '', '-f', hostKey]);
	await runProgram('/usr/bin/ssh-keygen', ['-q', '-t', 'ed25519', '-N', '', '-f', clientKey]);
	const authorizedKeys = path.join(fixtureDirectory, 'authorized-keys');
	await writeFile(authorizedKeys, await readFile(`${clientKey}.pub`, 'utf8'), { mode: 0o600 });
	const port = await reserveLoopbackPort();
	const username = userInfo().username;
	const configuration = path.join(fixtureDirectory, 'sshd-config');
	await writeFile(
		configuration,
		[
			`Port ${port}`,
			'ListenAddress 127.0.0.1',
			`HostKey ${hostKey}`,
			`PidFile ${path.join(fixtureDirectory, 'sshd.pid')}`,
			`AuthorizedKeysFile ${authorizedKeys}`,
			'PasswordAuthentication no',
			'KbdInteractiveAuthentication no',
			'AcceptEnv LANG',
			'UsePAM no',
			'PermitRootLogin no',
			'StrictModes no',
			`AllowUsers ${username}`,
			'LogLevel ERROR',
			'Subsystem sftp internal-sftp',
			'',
		].join('\n'),
		{ mode: 0o600 },
	);
	await runProgram('/usr/sbin/sshd', ['-t', '-f', configuration]);
	const server = spawn('/usr/sbin/sshd', ['-D', '-e', '-f', configuration], {
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	let serverError = '';
	server.stderr.setEncoding('utf8');
	server.stderr.on('data', (chunk: string) => {
		serverError += chunk;
	});
	try {
		await expect
			.poll(
				async () => {
					if (server.exitCode !== null) {
						throw new Error(`Loopback sshd exited before readiness: ${serverError}`);
					}
					return canConnectToLoopbackPort(port);
				},
				{ timeout: 5_000 },
			)
			.toBe(true);
	} catch (error) {
		await stopChildProcess(server);
		throw error;
	}
	return { clientKey, port, server, username };
}

async function canConnectToLoopbackPort(port: number): Promise<boolean> {
	return new Promise((resolve) => {
		const socket = createConnection({ host: '127.0.0.1', port });
		socket.once('connect', () => {
			socket.destroy();
			resolve(true);
		});
		socket.once('error', () => resolve(false));
	});
}

async function runProgram(executable: string, arguments_: readonly string[]): Promise<void> {
	const child = spawn(executable, arguments_, { stdio: ['ignore', 'pipe', 'pipe'] });
	let output = '';
	child.stdout.setEncoding('utf8');
	child.stderr.setEncoding('utf8');
	child.stdout.on('data', (chunk: string) => {
		output += chunk;
	});
	child.stderr.on('data', (chunk: string) => {
		output += chunk;
	});
	const exitCode = await new Promise<number | null>((resolve, reject) => {
		child.once('error', reject);
		child.once('exit', resolve);
	});
	if (exitCode !== 0) {
		throw new Error(`${executable} exited with ${String(exitCode)}: ${output}`);
	}
}

async function stopChildProcess(process_: ChildProcessWithoutNullStreams): Promise<void> {
	if (process_.exitCode !== null) return;
	process_.kill('SIGTERM');
	await Promise.race([
		new Promise<void>((resolve) => process_.once('exit', () => resolve())),
		wait(2_000).then(() => undefined),
	]);
	if (process_.exitCode !== null) return;
	process_.kill('SIGKILL');
	await Promise.race([
		new Promise<void>((resolve) => process_.once('exit', () => resolve())),
		wait(2_000).then(() => undefined),
	]);
}

async function openTerminalActionsWitness(): Promise<Page> {
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(new URL('terminal-actions-fixture.html', currentDevelopmentRendererUrl).href);
		return page;
	} catch (error) {
		await browser.close();
		throw error;
	}
}

async function openTerminalUrlPolicyWitness(): Promise<Page> {
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-url-policy-fixture.html', currentDevelopmentRendererUrl).href,
		);
		return page;
	} catch (error) {
		await browser.close();
		throw error;
	}
}

async function openTerminalClipboardCapabilityWitness(): Promise<Page> {
	const currentDevelopmentRendererUrl = developmentRendererUrl;
	if (currentDevelopmentRendererUrl === undefined) {
		throw new Error('Web development renderer server is unavailable.');
	}
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ viewport: { height: 500, width: 900 } });
		await page.goto(
			new URL('terminal-clipboard-capability-fixture.html', currentDevelopmentRendererUrl).href,
		);
		return page;
	} catch (error) {
		await browser.close();
		throw error;
	}
}

function quotePosixShellWord(value: string): string {
	return `'${value.replaceAll("'", "'\\''")}'`;
}

async function startWebRendererServer(): Promise<{
	readonly server: Server;
	readonly url: string;
}> {
	const server = createServer((request, response) => {
		void serveWebRendererRequest(request, response);
	});
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', () => {
			server.off('error', reject);
			resolve();
		});
	});
	const address = server.address() as AddressInfo | null;
	if (address === null) {
		server.close();
		throw new Error('Web renderer server did not publish an address.');
	}
	return { server, url: `http://127.0.0.1:${address.port}/` };
}

async function startWebRendererDevelopmentServer(): Promise<{
	readonly server: RsbuildDevServer;
	readonly url: string;
}> {
	const port = await reserveLoopbackPort();
	const loaded = await loadConfig({ command: 'dev', cwd: webRoot });
	const config = mergeRsbuildConfig(loaded.content, {
		server: { host: '127.0.0.1', port, strictPort: true },
		source: {
			entry: {
				index: './src/composition/entry.ts',
				'panel-broadcast-input-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/panel-broadcast-input.system-fixture.ts',
				'terminal-bell-effects-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/bell-effects.system-fixture.ts',
				'terminal-dpr-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/dpr.system-fixture.ts',
				'terminal-resize-ownership-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/resize-ownership.system-fixture.ts',
				'terminal-input-repair-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/input-repair.system-fixture.ts',
				'terminal-input-capability-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/input-capability.system-fixture.ts',
				'terminal-actions-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/terminal-actions.system-fixture.ts',
				'terminal-shared-focus-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/shared-focus.system-fixture.ts',
				'terminal-url-policy-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/url-policy.system-fixture.ts',
				'terminal-clipboard-capability-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/clipboard-capability.system-fixture.ts',
				'terminal-presentation-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/runtime-presentation.system-fixture.ts',
				'terminal-visibility-fixture':
					'./src/workbench-client/projected-terminal-renderer/__tests__/visibility.system-fixture.ts',
			},
		},
	});
	const rsbuild = await createRsbuild({
		callerName: 'orchbee-desktop-system-test',
		cwd: webRoot,
		rsbuildConfig: config,
	});
	const initialCompilation = new Promise<Error | undefined>((resolve) => {
		rsbuild.onAfterDevCompile(({ isFirstCompile, stats }) => {
			if (!isFirstCompile) return;
			resolve(
				stats.hasErrors()
					? new Error(
							`Web development compilation failed.\n${stats.toString({ all: false, errorDetails: true, errors: true })}`,
						)
					: undefined,
			);
		});
	});
	const started = await rsbuild.startDevServer({ getPortSilently: true });
	const compilationError = await initialCompilation;
	if (compilationError !== undefined) {
		await started.server.close();
		throw compilationError;
	}
	return { server: started.server, url: `http://127.0.0.1:${started.port}/` };
}

async function reserveLoopbackPort(): Promise<number> {
	const server = createServer();
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', () => {
			server.off('error', reject);
			resolve();
		});
	});
	const address = server.address() as AddressInfo | null;
	if (address === null) {
		await closeWebRendererServer(server);
		throw new Error('Loopback port reservation did not publish an address.');
	}
	const port = address.port;
	await closeWebRendererServer(server);
	return port;
}

async function closeWebRendererServer(server: Server): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		server.close((error) => (error === undefined ? resolve() : reject(error)));
	});
}

async function serveWebRendererRequest(
	request: Pick<IncomingMessage, 'headers' | 'method' | 'url'>,
	response: ServerResponse,
): Promise<void> {
	try {
		const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname;
		const relativePath = pathname === '/' ? 'index.html' : decodeURIComponent(pathname.slice(1));
		const filePath = path.resolve(webDist, relativePath);
		if (filePath !== webDist && !filePath.startsWith(`${webDist}${path.sep}`)) {
			response.statusCode = 400;
			response.end();
			return;
		}
		const body = await readFile(filePath);
		response.statusCode = 200;
		response.setHeader('content-type', contentTypeFor(filePath));
		response.end(body);
	} catch {
		await serveWebRendererDocumentFallback(request, response);
	}
}

async function serveWebRendererDocumentFallback(
	request: Pick<IncomingMessage, 'headers' | 'method' | 'url'>,
	response: ServerResponse,
): Promise<void> {
	const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname;
	const isReservedNamespace = ['/api', '/static', '/ws'].some(
		(namespace) => pathname === namespace || pathname.startsWith(`${namespace}/`),
	);
	const acceptsHtml = request.headers.accept?.toLowerCase().includes('text/html') ?? false;
	if (request.method !== 'GET' || isReservedNamespace || !acceptsHtml) {
		response.statusCode = 404;
		response.end();
		return;
	}
	const body = await readFile(path.join(webDist, 'index.html'));
	response.statusCode = 200;
	response.setHeader('cache-control', 'no-cache');
	response.setHeader('content-type', 'text/html; charset=utf-8');
	response.end(body);
}

function contentTypeFor(filePath: string): string {
	switch (path.extname(filePath)) {
		case '.css':
			return 'text/css; charset=utf-8';
		case '.html':
			return 'text/html; charset=utf-8';
		case '.js':
			return 'text/javascript; charset=utf-8';
		case '.ttf':
			return 'font/ttf';
		default:
			return 'application/octet-stream';
	}
}

async function startHost(
	directory: string,
	endpointId: string,
): Promise<ChildProcessWithoutNullStreams> {
	const process = spawn(
		hostBinary,
		['serve', '--runtime-directory', directory, '--endpoint-id', endpointId],
		{ stdio: ['pipe', 'pipe', 'pipe'] },
	);
	if (globalThis.process.platform === 'win32') {
		await wait(250);
		return process;
	}
	const endpoint = path.join(directory, `${endpointId}.s`);
	await expect
		.poll(
			async () =>
				access(endpoint)
					.then(() => true)
					.catch(() => false),
			{ timeout: 5_000 },
		)
		.toBe(true);
	return process;
}

async function quitDesktopWithChoice(
	application: ElectronApplication,
	page: Awaited<ReturnType<ElectronApplication['firstWindow']>>,
	choice: 'preserve' | 'stop',
): Promise<void> {
	const exited = waitForElectronExit(application);
	await application.evaluate(({ app }) => app.quit());
	const button = page.locator(`[data-lifetime-choice="${choice}"]`);
	await expect(button).toBeVisible();
	await expect(page.locator('[data-lifetime-choice="preserve"]')).toBeFocused();
	await button.click();
	await waitForDesktopExit(exited, page, choice);
}

async function quitDesktopAutomaticallyPreservingSessions(
	application: ElectronApplication,
): Promise<void> {
	const exited = waitForElectronExit(application);
	await application.evaluate(({ app }) => app.quit());
	await waitForDesktopExit(exited, undefined, 'automatic preserve');
}

async function quitDesktopPreservingSessions(
	application: ElectronApplication,
	page: Awaited<ReturnType<ElectronApplication['firstWindow']>>,
): Promise<void> {
	const exited = waitForElectronExit(application);
	await application.evaluate(({ app }) => app.quit());
	const preserve = page.locator('[data-lifetime-choice="preserve"]');
	const outcome = await Promise.race([
		exited.then(() => 'exited' as const),
		preserve
			.waitFor({ state: 'visible' })
			.then(() => 'choice' as const)
			.catch(() => 'renderer-retired' as const),
	]);
	if (outcome === 'choice') {
		await expect(preserve).toBeFocused();
		await preserve.click();
	}
	await waitForDesktopExit(exited, page, 'preserve');
}

async function waitForDesktopExit(
	exited: Promise<void>,
	page: Page | undefined,
	mode: string,
): Promise<void> {
	await Promise.race([
		exited,
		wait(10_000).then(async () => {
			const visibleState =
				page === undefined
					? 'renderer state not requested'
					: await page
							.locator('body')
							.innerText()
							.catch(() => 'renderer-unavailable');
			throw new Error(`Desktop did not exit after ${mode}: ${visibleState}`);
		}),
	]);
}

async function forceCloseDesktop(application: ElectronApplication): Promise<void> {
	let runningProcess: ReturnType<ElectronApplication['process']>;
	try {
		runningProcess = application.process();
	} catch {
		return;
	}
	const exited = waitForElectronExit(application);
	runningProcess.kill('SIGKILL');
	await Promise.race([exited, wait(2_000).then(() => undefined)]);
}

function waitForElectronExit(application: ElectronApplication): Promise<void> {
	let process: ReturnType<ElectronApplication['process']>;
	try {
		process = application.process();
	} catch {
		return Promise.resolve();
	}
	if (process.exitCode !== null) return Promise.resolve();
	return new Promise((resolve) => process.once('exit', () => resolve()));
}

function projectedTerminal(page: Page): Locator {
	return page.locator('[data-terminal-presentation-id][data-projected-terminal-state]').first();
}

interface TerminalPresentationFrameSample {
	readonly exposedGeometryStates: readonly (string | null)[];
	readonly exposedMode: string | null;
	readonly layerModes: readonly (string | null)[];
	readonly timestamp: number;
}

const TERMINAL_PRESENTATION_FRAME_TRACE_KEY = '__orchbeeTerminalPresentationFrameTrace';

async function startTerminalPresentationFrameTrace(page: Page): Promise<void> {
	await page.evaluate((traceKey) => {
		interface TraceState {
			samples: TerminalPresentationFrameSample[];
			stopped: boolean;
		}
		const previous = Reflect.get(globalThis, traceKey) as TraceState | undefined;
		if (previous !== undefined) previous.stopped = true;
		const state: TraceState = { samples: [], stopped: false };
		Reflect.set(globalThis, traceKey, state);
		const sample = (timestamp: number): void => {
			const workspace = [
				...document.querySelectorAll<HTMLElement>('[data-slot="workbench-panel-workspace"]'),
			].find((candidate) => {
				const bounds = candidate.getBoundingClientRect();
				return bounds.width > 0 && bounds.height > 0;
			});
			const layers =
				workspace === undefined
					? []
					: [...workspace.querySelectorAll<HTMLElement>(':scope > [data-presentation-mode]')];
			const exposedLayer =
				layers.find((layer) => layer.dataset['presentationMode'] === 'retained') ??
				layers.find((layer) => layer.dataset['presentationMode'] === 'active') ??
				layers.find((layer) => layer.dataset['presentationMode'] === 'incoming');
			const terminalRoots =
				exposedLayer === undefined
					? []
					: [
							...exposedLayer.querySelectorAll<HTMLElement>(
								'[data-terminal-presentation-id][data-projected-terminal-state]',
							),
						];
			state.samples.push({
				exposedGeometryStates: terminalRoots.map(
					(root) => root.dataset['projectedTerminalPresentationGeometryCurrent'] ?? null,
				),
				exposedMode: exposedLayer?.dataset['presentationMode'] ?? null,
				layerModes: layers.map((layer) => layer.dataset['presentationMode'] ?? null),
				timestamp,
			});
			if (!state.stopped) globalThis.requestAnimationFrame(sample);
		};
		globalThis.requestAnimationFrame(sample);
	}, TERMINAL_PRESENTATION_FRAME_TRACE_KEY);
}

async function stopTerminalPresentationFrameTrace(
	page: Page,
): Promise<readonly TerminalPresentationFrameSample[]> {
	return page.evaluate(async (traceKey) => {
		interface TraceState {
			samples: TerminalPresentationFrameSample[];
			stopped: boolean;
		}
		const state = Reflect.get(globalThis, traceKey) as TraceState | undefined;
		if (state === undefined) throw new Error('Terminal presentation frame trace is unavailable.');
		state.stopped = true;
		await new Promise<void>((resolve) => globalThis.requestAnimationFrame(() => resolve()));
		Reflect.deleteProperty(globalThis, traceKey);
		return state.samples;
	}, TERMINAL_PRESENTATION_FRAME_TRACE_KEY);
}

function expectAtomicTerminalPresentationTrace(
	samples: readonly TerminalPresentationFrameSample[],
): void {
	expect(samples.length).toBeGreaterThan(0);
	expect(
		samples.some(
			(sample) => sample.layerModes.includes('incoming') && sample.layerModes.includes('retained'),
		),
	).toBe(true);
	expect(
		samples.filter(
			(sample) =>
				sample.exposedMode === null ||
				sample.exposedGeometryStates.length === 0 ||
				sample.exposedGeometryStates.some((state) => state !== 'true'),
		),
	).toEqual([]);
}

async function dispatchDesktopFileDrop(terminalRoot: Locator): Promise<{
	readonly bridgePath: string;
	readonly dragOverPrevented: boolean;
	readonly dropFileCount: number;
	readonly dropItemCount: number;
	readonly dropPrevented: boolean;
	readonly dropTypes: readonly string[];
}> {
	return await terminalRoot.evaluate((element) => {
		const input = document.querySelector('input[data-desktop-file-drop-fixture="true"]');
		const file = input instanceof HTMLInputElement ? input.files?.[0] : undefined;
		if (file === undefined) throw new Error('Desktop file-drop fixture has no selected file.');
		const bridge: unknown = Reflect.get(window, 'orchbeeDesktopFilePath');
		if (
			typeof bridge !== 'object' ||
			bridge === null ||
			!('getPathForFile' in bridge) ||
			typeof bridge.getPathForFile !== 'function'
		) {
			throw new Error('Desktop file-drop fixture has no path bridge.');
		}
		const bridgePath: unknown = Reflect.apply(bridge.getPathForFile, bridge, [file]);
		if (typeof bridgePath !== 'string') {
			throw new Error('Desktop file-drop fixture path bridge returned a non-string value.');
		}
		const dragTransfer = {
			dropEffect: 'none',
			files: [],
			getData: () => '',
			items: [{ getAsFile: () => null, kind: 'file' }],
			types: ['Files'],
		};
		const dragOver = new Event('dragover', { bubbles: true, cancelable: true });
		Object.defineProperty(dragOver, 'dataTransfer', { value: dragTransfer });
		element.dispatchEvent(dragOver);

		const dropTransfer = {
			dropEffect: 'none',
			files: [file],
			getData: () => '',
			items: [{ getAsFile: () => file, kind: 'file' }],
			types: ['Files'],
		};
		const drop = new Event('drop', { bubbles: true, cancelable: true });
		Object.defineProperty(drop, 'dataTransfer', { value: dropTransfer });
		element.dispatchEvent(drop);
		return {
			bridgePath,
			dragOverPrevented: dragOver.defaultPrevented,
			dropFileCount: dropTransfer.files.length,
			dropItemCount: dropTransfer.items.length,
			dropPrevented: drop.defaultPrevented,
			dropTypes: Array.from(dropTransfer.types),
		};
	});
}

async function dprWitness(root: Locator): Promise<{
	readonly atlasClearCount: number;
	readonly atlasClearDelta: number;
	readonly canvasAlpha: number;
	readonly canvasCssHeight: number;
	readonly canvasCssWidth: number;
	readonly canvasFillTextDelta: number;
	readonly canvasHeight: number;
	readonly canvasWidth: number;
	readonly columns: number;
	readonly devicePixelRatio: number;
	readonly inputAnchorCoherent: boolean;
	readonly inputGeometry: string;
	readonly lastCellsVisited: number;
	readonly lastForceAll: boolean;
	readonly lastGeneration: string;
	readonly lastReasons: string;
	readonly lastRevision: string;
	readonly lastRowsRendered: number;
	readonly matchMediaActiveCount: number;
	readonly matchMediaCallCount: number;
	readonly matchMediaInvocationCount: number;
	readonly matchMediaLastQuery: string;
	readonly matchMediaRemoveCount: number;
	readonly renderCount: number;
	readonly renderDelta: number;
	readonly resizeCellPixelHeight: number;
	readonly resizeCellPixelWidth: number;
	readonly resizeColumns: number;
	readonly resizeCount: number;
	readonly resizeDelta: number;
	readonly resizeRows: number;
	readonly resizeTerminalPixelHeight: number;
	readonly resizeTerminalPixelWidth: number;
	readonly rows: number;
	readonly terminalMutationCount: number;
	readonly windowResizeActiveCount: number;
	readonly windowResizeInvocationCount: number;
}> {
	return root.evaluate((element) => {
		if (!(element instanceof HTMLElement)) throw new Error('DPR witness root is invalid.');
		const data = element.dataset;
		const current = data['witnessCurrentDpr'] === undefined ? 'Initial' : 'Current';
		const number = (key: string): number => Number(data[key] ?? '0');
		return {
			atlasClearCount: number('witnessAtlasClearCount'),
			atlasClearDelta: number('witnessAtlasClearDelta'),
			canvasAlpha: number('witnessCurrentCanvasAlpha'),
			canvasCssHeight: number(`witness${current}CanvasCssHeight`),
			canvasCssWidth: number(`witness${current}CanvasCssWidth`),
			canvasFillTextDelta: number('witnessCanvasFillTextDelta'),
			canvasHeight: number(`witness${current}CanvasHeight`),
			canvasWidth: number(`witness${current}CanvasWidth`),
			columns: number(`witness${current}Columns`),
			devicePixelRatio: number(`witness${current}Dpr`),
			inputAnchorCoherent: data['witnessInputAnchorCoherent'] === 'true',
			inputGeometry: data[`witness${current}InputGeometry`] ?? '',
			lastCellsVisited: number('witnessLastCellsVisited'),
			lastForceAll: data['witnessLastForceAll'] === 'true',
			lastGeneration: data['witnessLastGeneration'] ?? '',
			lastReasons: data['witnessLastReasons'] ?? '',
			lastRevision: data['witnessLastRevision'] ?? '',
			lastRowsRendered: number('witnessLastRowsRendered'),
			matchMediaActiveCount: number('witnessMatchMediaActiveCount'),
			matchMediaCallCount: number('witnessMatchMediaCallCount'),
			matchMediaInvocationCount: number('witnessMatchMediaInvocationCount'),
			matchMediaLastQuery: data['witnessMatchMediaLastQuery'] ?? '',
			matchMediaRemoveCount: number('witnessMatchMediaRemoveCount'),
			renderCount: number('witnessRenderCount'),
			renderDelta: number('witnessRenderDelta'),
			resizeCellPixelHeight: number('witnessResizeCellPixelHeight'),
			resizeCellPixelWidth: number('witnessResizeCellPixelWidth'),
			resizeColumns: number('witnessResizeColumns'),
			resizeCount: number('witnessResizeCount'),
			resizeDelta: number('witnessResizeDelta'),
			resizeRows: number('witnessResizeRows'),
			resizeTerminalPixelHeight: number('witnessResizeTerminalPixelHeight'),
			resizeTerminalPixelWidth: number('witnessResizeTerminalPixelWidth'),
			rows: number(`witness${current}Rows`),
			terminalMutationCount: number('witnessTerminalMutationCount'),
			windowResizeActiveCount: number('witnessWindowResizeActiveCount'),
			windowResizeInvocationCount: number('witnessWindowResizeInvocationCount'),
		};
	});
}

async function runtimePresentationWitness(root: Locator): Promise<{
	readonly backend: string | null;
	readonly canvasAlpha: number;
	readonly canvasHeight: string;
	readonly canvasMargin: string;
	readonly canvasWidth: string;
	readonly inputFontFamily: string;
	readonly inputFontSize: string;
	readonly inputFontWeight: string;
	readonly inputLeft: string;
	readonly inputLigatures: string;
	readonly inputTop: string;
	readonly inputValue: string;
	readonly inputWidth: string;
	readonly presentationCommittedEpoch: string | undefined;
	readonly presentationDrawnEpoch: string | undefined;
	readonly renderedFrames: number;
}> {
	return root.evaluate((element) => {
		const canvas = element.querySelector('canvas');
		const input = element.querySelector('textarea');
		if (!(canvas instanceof HTMLCanvasElement) || !(input instanceof HTMLTextAreaElement)) {
			throw new Error('Runtime presentation witness surface is incomplete.');
		}
		const context = canvas.getContext('2d');
		if (context === null) throw new Error('Runtime presentation witness requires Canvas 2D.');
		const sample = context.getImageData(
			Math.max(0, canvas.width - 2),
			Math.max(0, canvas.height - 2),
			1,
			1,
		).data;
		return {
			backend: element.getAttribute('data-projected-terminal-renderer'),
			canvasAlpha: sample[3] ?? -1,
			canvasHeight: canvas.style.height,
			canvasMargin: [
				canvas.style.marginTop,
				canvas.style.marginRight,
				canvas.style.marginBottom,
				canvas.style.marginLeft,
			].join(' '),
			canvasWidth: canvas.style.width,
			inputFontFamily: input.style.fontFamily,
			inputFontSize: input.style.fontSize,
			inputFontWeight: input.style.fontWeight,
			inputLeft: input.style.left,
			inputLigatures: input.style.fontVariantLigatures,
			inputTop: input.style.top,
			inputValue: input.value,
			inputWidth: input.style.width,
			presentationCommittedEpoch: element.dataset['projectedTerminalPresentationCommittedEpoch'],
			presentationDrawnEpoch: element.dataset['projectedTerminalPresentationDrawnEpoch'],
			renderedFrames: Number(element.dataset['projectedTerminalRenderedFrames'] ?? '0'),
		};
	});
}

async function runtimeThemeWitness(root: Locator): Promise<{
	readonly applicationTheme: string | undefined;
	readonly canvasRgb: string;
	readonly renderedFrames: number;
	readonly themeRevision: string | undefined;
}> {
	return root.evaluate((element) => {
		const canvas = element.querySelector('canvas');
		if (!(canvas instanceof HTMLCanvasElement)) {
			throw new Error('Runtime theme witness surface is incomplete.');
		}
		const context = canvas.getContext('2d');
		if (context === null) throw new Error('Runtime theme witness requires Canvas 2D.');
		const sample = context.getImageData(
			Math.max(0, canvas.width - 2),
			Math.max(0, canvas.height - 2),
			1,
			1,
		).data;
		return {
			applicationTheme: document.documentElement.dataset['theme'],
			canvasRgb: [sample[0], sample[1], sample[2]].join(','),
			renderedFrames: Number(element.dataset['projectedTerminalRenderedFrames'] ?? '0'),
			themeRevision: element.dataset['projectedTerminalThemeRevision'],
		};
	});
}

function projectedTerminals(page: Page): Locator {
	return page.locator('[data-terminal-presentation-id][data-projected-terminal-state]');
}

function projectedTerminalFor(page: Page, presentationId: string): Locator {
	return page.locator(
		`[data-terminal-presentation-id="${presentationId}"][data-projected-terminal-state]`,
	);
}

function workbenchTabs(page: Page): Locator {
	return page.locator('[data-slot="workbench-tab-bar"] [role="tab"]');
}

function workbenchWorkspace(page: Page, name: string): Locator {
	return page.locator('[data-slot="workbench-sidebar-workspace"]').filter({ hasText: name });
}

async function expandProject(page: Page, name: string): Promise<void> {
	const project = page.locator('[data-slot="workbench-sidebar-project"]').filter({ hasText: name });
	await expect(project).toBeVisible();
	if ((await project.getAttribute('aria-expanded')) !== 'true') await project.click();
	await expect(project).toHaveAttribute('aria-expanded', 'true');
}

async function expectVisibleLayout(locator: Locator): Promise<void> {
	await expect(locator).toBeVisible();
	const bounds = await locator.boundingBox();
	expect(bounds).not.toBeNull();
	expect(bounds?.width).toBeGreaterThan(0);
	expect(bounds?.height).toBeGreaterThan(0);
}

async function expectTerminalSurfaceAppearance(locator: Locator): Promise<void> {
	await expect
		.poll(() =>
			locator.evaluate((element) => {
				const style = getComputedStyle(element);
				return {
					backgroundColor: style.backgroundColor,
					borderBottomWidth: style.borderBottomWidth,
					borderLeftWidth: style.borderLeftWidth,
					borderRightWidth: style.borderRightWidth,
					borderTopWidth: style.borderTopWidth,
					paddingBottom: style.paddingBottom,
					paddingLeft: style.paddingLeft,
					paddingRight: style.paddingRight,
					paddingTop: style.paddingTop,
					statusPosition: getComputedStyle(
						element.querySelector('[data-projected-terminal-status="true"]') ?? element,
					).position,
				};
			}),
		)
		.toEqual({
			backgroundColor: 'rgb(252, 252, 252)',
			borderBottomWidth: '0px',
			borderLeftWidth: '1px',
			borderRightWidth: '1px',
			borderTopWidth: '0px',
			paddingBottom: '0px',
			paddingLeft: '0px',
			paddingRight: '0px',
			paddingTop: '0px',
			statusPosition: 'absolute',
		});
}

async function desktopClientSessionIds(page: Page): Promise<readonly string[]> {
	return page.evaluate(() => {
		const value = JSON.parse(
			sessionStorage.getItem('orchbee-system-test-client-session-ids') ?? '[]',
		) as unknown;
		return Array.isArray(value)
			? value.filter((candidate): candidate is string => typeof candidate === 'string')
			: [];
	});
}

async function waitForProjectedTerminalsOpen(page: Page): Promise<void> {
	const terminals = projectedTerminals(page);
	await expect
		.poll(async () => {
			const states = await terminals.evaluateAll((elements) =>
				elements.map((element) => element.getAttribute('data-projected-terminal-state')),
			);
			return states.length > 0 && states.every((state) => state === 'open');
		})
		.toBe(true);
}

async function projectedTerminalPresentationIds(page: Page): Promise<readonly string[]> {
	return projectedTerminals(page).evaluateAll((elements) =>
		elements.flatMap((element) => {
			const presentationId = element.getAttribute('data-terminal-presentation-id');
			return presentationId === null ? [] : [presentationId];
		}),
	);
}

interface GatewaySocketTrace {
	readonly events: string[];
	readonly urls: string[];
}

function trackGatewaySockets(page: Page): GatewaySocketTrace {
	const trace: GatewaySocketTrace = { events: [], urls: [] };
	page.on('console', (message) => trace.events.push(`console:${message.type()}:${message.text()}`));
	page.on('pageerror', (error) => trace.events.push(`pageerror:${error.message}`));
	page.on('websocket', (socket) => {
		const pathname = new URL(socket.url()).pathname;
		trace.urls.push(socket.url());
		trace.events.push(`socket:${pathname}:opened`);
		socket.on('framesent', (event) =>
			trace.events.push(`socket:${pathname}:sent:${gatewayFrameShape(event.payload)}`),
		);
		socket.on('framereceived', (event) =>
			trace.events.push(`socket:${pathname}:received:${gatewayFrameShape(event.payload)}`),
		);
		socket.on('socketerror', (error) => trace.events.push(`socket:${pathname}:error:${error}`));
		socket.on('close', () => trace.events.push(`socket:${pathname}:closed`));
	});
	return trace;
}

function gatewaySocketCount(trace: GatewaySocketTrace, pathname: string): number {
	return trace.urls.filter((url) => new URL(url).pathname === pathname).length;
}

function gatewaySocketEventCount(trace: GatewaySocketTrace, event: string): number {
	return trace.events.filter((candidate) => candidate === event).length;
}

function socketUsesRendererOrigin(socketUrl: string, rendererSourceUrl: string): boolean {
	const socket = new URL(socketUrl);
	const renderer = new URL(rendererSourceUrl);
	return socket.hostname === renderer.hostname && socket.port === renderer.port;
}

function socketUsesGatewayConnection(socketUrl: string, rendererSourceUrl: string): boolean {
	const socket = new URL(socketUrl);
	const renderer = new URL(rendererSourceUrl);
	const connection = decodeGatewayBrowserSearch(renderer.search);
	return (
		connection !== null &&
		socket.hostname === renderer.hostname &&
		socket.port === String(connection.port) &&
		socket.pathname === GATEWAY_CONTROL_PATH
	);
}

function isRfc1918IPv4(value: string): boolean {
	const octets = value.split('.').map(Number);
	if (octets.length !== 4 || octets.some((octet) => !Number.isInteger(octet))) return false;
	const first = octets[0];
	const second = octets[1];
	return (
		first === 10 ||
		(first === 172 && second !== undefined && second >= 16 && second <= 31) ||
		(first === 192 && second === 168)
	);
}

function gatewayFrameShape(payload: string | Buffer): string {
	if (typeof payload !== 'string') {
		const frame = payload.buffer.slice(
			payload.byteOffset,
			payload.byteOffset + payload.byteLength,
		) as ArrayBuffer;
		const client = decodeGatewayAttachmentClientFrame(frame);
		if (client.ok) return `binary:${payload.byteLength}:client:${client.value.kind}`;
		const server = decodeGatewayAttachmentServerFrame(frame);
		if (server.ok) return `binary:${payload.byteLength}:server:${server.value.kind}`;
		return `binary:${payload.byteLength}`;
	}
	try {
		const value: unknown = JSON.parse(payload);
		if (typeof value === 'object' && value !== null && 'type' in value) {
			const type = String(value.type);
			if (type === 'gateway-control-ready' && 'bootstrap' in value) {
				const bootstrap = value.bootstrap;
				if (
					typeof bootstrap === 'object' &&
					bootstrap !== null &&
					'documentState' in bootstrap &&
					typeof bootstrap.documentState === 'object' &&
					bootstrap.documentState !== null &&
					'kind' in bootstrap.documentState
				) {
					return `text:${Buffer.byteLength(payload)}:${type}:${String(bootstrap.documentState.kind)}`;
				}
			}
			return `text:${Buffer.byteLength(payload)}:${type}`;
		}
	} catch {
		return `text:${Buffer.byteLength(payload)}:invalid-json`;
	}
	return `text:${Buffer.byteLength(payload)}`;
}

async function terminalDocumentIdentity(root: Locator): Promise<{
	readonly attachmentId: string | null;
	readonly creationId: string | null;
	readonly hostEpoch: string | null;
	readonly terminalId: string | null;
}> {
	return {
		attachmentId: await root.getAttribute('data-projected-terminal-attachment-id'),
		creationId: await root.getAttribute('data-projected-terminal-creation-id'),
		hostEpoch: await root.getAttribute('data-projected-terminal-host-epoch'),
		terminalId: await root.getAttribute('data-projected-terminal-id'),
	};
}

async function terminalCellPoint(
	root: Locator,
	canvas: Locator,
	rows: Locator,
	text: string,
	columnOffset: number,
	horizontalFraction = 0.5,
): Promise<{ readonly x: number; readonly y: number }> {
	const row = rows.filter({ hasText: text }).first();
	const location = await row.evaluate(
		(element, expectedText) => ({
			column: (element.textContent ?? '').indexOf(expectedText),
			row: Array.from(element.parentElement?.children ?? []).indexOf(element),
		}),
		text,
	);
	if (location.column < 0 || location.row < 0) {
		throw new Error(`Terminal text ${text} is not visible.`);
	}
	const bounds = await canvas.boundingBox();
	if (bounds === null) throw new Error('Terminal canvas has no bounding box.');
	const columns = Number(await root.getAttribute('data-projected-terminal-columns'));
	const visibleRows = Number(await root.getAttribute('data-projected-terminal-rows'));
	return {
		x: bounds.x + (location.column + columnOffset + horizontalFraction) * (bounds.width / columns),
		y: bounds.y + (location.row + 0.5) * (bounds.height / visibleRows),
	};
}

async function terminalDocumentSelectionState(
	root: Locator,
): Promise<'collapsed' | 'none' | 'outside' | 'terminal-range'> {
	return root.evaluate((element) => {
		const selection = element.ownerDocument.getSelection();
		if (selection === null || selection.rangeCount === 0) return 'none';
		if (selection.isCollapsed) return 'collapsed';
		const accessibility = element.querySelector('[role="list"]');
		const range = selection.getRangeAt(0);
		return accessibility?.contains(range.startContainer) === true ||
			accessibility?.contains(range.endContainer) === true
			? 'terminal-range'
			: 'outside';
	});
}

async function waitForTerminalPresentationStability(root: Locator): Promise<void> {
	let previousSignature: string | undefined;
	let consecutiveMatches = 0;
	await expect
		.poll(
			async () => {
				const signature = await root.evaluate((element) =>
					JSON.stringify({
						committedFrames: element.getAttribute('data-projected-terminal-committed-frames'),
						log: element.querySelector('[role="list"]')?.textContent ?? '',
						renderedFrames: element.getAttribute('data-projected-terminal-rendered-frames'),
					}),
				);
				if (signature === previousSignature) consecutiveMatches += 1;
				else {
					previousSignature = signature;
					consecutiveMatches = 0;
				}
				return consecutiveMatches;
			},
			{ intervals: [50], timeout: 10_000 },
		)
		.toBeGreaterThanOrEqual(2);
}

async function captureTerminalScrollbarStrip(page: Page, surface: Locator): Promise<Buffer> {
	const bounds = await surface.boundingBox();
	if (bounds === null) throw new Error('Terminal surface has no bounding box.');
	return page.screenshot({
		clip: {
			height: bounds.height,
			width: Math.min(16, bounds.width),
			x: bounds.x + Math.max(0, bounds.width - 16),
			y: bounds.y,
		},
	});
}
