import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { createConnection, type AddressInfo } from 'node:net';
import { tmpdir, userInfo } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

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
	decodeGatewayBrowserFragment,
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
const startupGeometryWitnessTitle = 'terminal-parity/SU-16-startup-geometry';
const MANUAL_NATIVE_WINDOW_TEST_TAG = '@manual-native-window';
const MANUAL_NATIVE_WINDOW_TEST_APPROVAL = 'I_ACKNOWLEDGE_FOCUS_INTERRUPTION';

const applicationOutput = new WeakMap<ElectronApplication, string[]>();

let electronApplication: ElectronApplication | undefined;
let desktopEnvironment: Record<string, string> | undefined;
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
	let shell = '/bin/sh';
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

	const environment = Object.fromEntries(
		Object.entries(process.env).filter(
			(entry): entry is [string, string] => entry[1] !== undefined,
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
			input.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					code: 'ShiftLeft',
					key: 'Shift',
					shiftKey: true,
				}),
			);
			const afterModifier = inputGeometry();
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
			};
		});
		expect(inputAnchorRecovery.beforeModifier.width).not.toBe('1px');
		expect(inputAnchorRecovery.afterModifier).toEqual(inputAnchorRecovery.beforeModifier);
		expect(inputAnchorRecovery.activeElementRetained).toBe(true);
		expect(inputAnchorRecovery.beforeWindowFocus.width).not.toBe('1px');
		expect(inputAnchorRecovery.afterWindowFocus).toEqual(inputAnchorRecovery.beforeWindowFocus);
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

test('terminal-parity/LC-04-environment', async () => {
	test.skip(process.platform === 'win32', 'The launch-locale witness is POSIX-only.');
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const page = await application.firstWindow();
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'open');

	const input = root.getByRole('textbox');
	const command =
		'printf \'orchbee-launch-env TERM=%s COLORTERM=%s TERM_PROGRAM=%s NO_COLOR=%s FORCE_COLOR=%s NODE_DISABLE_COLORS=%s CHARMAP=%s\\n\' "$TERM" "$COLORTERM" "$TERM_PROGRAM" "${NO_COLOR-unset}" "${FORCE_COLOR-unset}" "${NODE_DISABLE_COLORS-unset}" "$(locale charmap 2>/dev/null || printf unknown)"';
	await input.pressSequentially(command, { delay: 1 });
	await input.press('Enter');
	await expect(root.locator('[role="list"]')).toContainText(
		/orchbee-launch-env TERM=xterm-256color COLORTERM=truecolor TERM_PROGRAM=Orchbee NO_COLOR=unset FORCE_COLOR=unset NODE_DISABLE_COLORS=unset CHARMAP=UTF-?8/u,
	);
});

test('runs an ordinary SSH PTY with a diagnosable TERM and no implicit Orchbee environment forwarding', async () => {
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

test('restores an equal four-Panel grid after closing a repeated-axis nested split', async () => {
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
	await expect(
		projectedTerminalFor(page, bottomLeftPresentationId).getByRole('textbox'),
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
	test.setTimeout(60_000);
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
			origins = await dialog.locator('code').allTextContents();
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
		await browserPage.goto(browserUrl);
		expect(new URL(browserPage.url()).origin).toBe(networkOrigin);
		try {
			const browserRoot = projectedTerminal(browserPage);
			await expect(browserRoot).toHaveAttribute('data-projected-terminal-state', 'open');
			await expectTerminalSurfaceAppearance(browserRoot);
		} catch (error: unknown) {
			throw new Error(`LAN Browser access failed: ${browserSockets.events.join(' | ')}`, {
				cause: error,
			});
		}
	} finally {
		await browser.close();
	}
	await dialog.locator('[data-browser-access-action="stop"]').click();
	await expect(dialog.locator('code')).toHaveCount(0);
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

	const gatewayUrl = await openLoopbackBrowserAccess(developmentApplication, desktopPage);
	const browser = await chromium.launch();
	try {
		const context = await browser.newContext({ viewport: { height: 720, width: 1_280 } });
		const browserPage = await context.newPage();
		browserPage.setDefaultTimeout(10_000);
		const browserSockets = trackGatewaySockets(browserPage);
		await browserPage.goto(gatewayUrl);
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
	const gatewayUrl = await openLoopbackBrowserAccess(application, desktopPage);
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
		await firstBrowserPage.goto(gatewayUrl);
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

		const rejectedPage = await context.newPage();
		let rejectedControlClosed = false;
		rejectedPage.on('websocket', (socket) => {
			if (new URL(socket.url()).pathname === '/v1/control') {
				socket.on('close', () => {
					rejectedControlClosed = true;
				});
			}
		});
		const rejectedUrl = withGatewayCapability(gatewayUrl, 'b'.repeat(43));
		await rejectedPage.goto(rejectedUrl);
		await expect.poll(() => rejectedControlClosed).toBe(true);
		await expect(projectedTerminal(rejectedPage)).toHaveCount(0);
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
	const gatewayUrl = await openLoopbackBrowserAccess(application, desktopPage);
	desktopPage.setDefaultTimeout(10_000);
	const browser = await chromium.launch();
	let browserContext = await browser.newContext();
	let browserPage = await browserContext.newPage();
	browserPage.setDefaultTimeout(10_000);
	const browserSockets = trackGatewaySockets(browserPage);

	try {
		await browserPage.goto(gatewayUrl);
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

		const restartedGatewayUrl = await openLoopbackBrowserAccess(application, desktopPage);
		browserContext = await browser.newContext();
		browserPage = await browserContext.newPage();
		browserPage.setDefaultTimeout(10_000);
		await browserPage.goto(restartedGatewayUrl);
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
	const application = await electron.launch({
		args: [`--user-data-dir=${path.join(runtimeDirectory, 'electron-user-data')}`, desktopMain],
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
): Promise<string> {
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
	return browserUrl;
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
	const connection = decodeGatewayBrowserFragment(renderer.hash);
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

function withGatewayCapability(browserUrl: string, capability: string): string {
	const url = new URL(browserUrl);
	const fragment = new URLSearchParams(url.hash.slice(1));
	fragment.set('orchbee-gateway-capability', capability);
	url.hash = fragment.toString();
	return url.href;
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
