import { createServer, type Server } from 'node:http';
import { type AddressInfo } from 'node:net';
import path from 'node:path';

import {
	createRsbuild,
	loadConfig,
	mergeRsbuildConfig,
	type RsbuildDevServer,
} from '@rsbuild/core';
import { chromium, expect, test, type Locator, type Page } from '@playwright/test';

const webRoot = path.resolve(import.meta.dirname, '..');

let developmentRendererServer: RsbuildDevServer | undefined;

let developmentRendererUrl: string | undefined;

test.beforeAll(async () => {
	const developmentRenderer = await startWebRendererDevelopmentServer();
	developmentRendererServer = developmentRenderer.server;
	developmentRendererUrl = developmentRenderer.url;
});

test.afterAll(async () => {
	await developmentRendererServer?.close();
	developmentRendererServer = undefined;
	developmentRendererUrl = undefined;
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
			'已将剪贴板文本粘贴到终端。|已清除终端输出。|终端目标已发生变化，无法打开。',
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

test('terminal-parity/FS-19-local-file-links', async () => {
	test.setTimeout(45_000);
	const page = await openTerminalUrlPolicyWitness();
	try {
		const root = page.locator('#terminal-url-policy-witness');
		await expect(root).toHaveAttribute('data-witness-ready', 'true');
		await expect(root).not.toHaveAttribute('data-witness-failure', /.+/u);
		await expect(root).toHaveAttribute('data-witness-local-ready', 'true');
		await expect(root).toHaveAttribute(
			'data-witness-local-opened-paths',
			'/fixture/images/final.png',
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
			'clipboard-write-unavailable',
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
			'clipboard-write-unavailable',
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
			'clipboard-write-unavailable',
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
		callerName: 'orchbee-web-system-test',
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
		await closeLoopbackServer(server);
		throw new Error('Loopback port reservation did not publish an address.');
	}
	const port = address.port;
	await closeLoopbackServer(server);
	return port;
}

async function closeLoopbackServer(server: Server): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		server.close((error) => (error === undefined ? resolve() : reject(error)));
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
