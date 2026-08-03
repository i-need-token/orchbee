import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer, type Server, type ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import {
	_electron as electron,
	chromium,
	expect,
	test,
	type ElectronApplication,
	type Locator,
	type Page,
} from '@playwright/test';

const repositoryRoot = path.resolve(import.meta.dirname, '..', '..', '..');
const hostBinary = path.join(
	repositoryRoot,
	'apps',
	'terminal-host',
	'zig-out',
	'bin',
	process.platform === 'win32' ? 'terminal-host.exe' : 'terminal-host',
);
const desktopMain = path.join(repositoryRoot, 'apps', 'desktop', 'dist', 'main', 'main.cjs');
const gatewayMain = path.join(repositoryRoot, 'apps', 'gateway', 'dist', 'main', 'gateway.cjs');
const webDist = path.join(repositoryRoot, 'apps', 'web', 'dist');

const applicationOutput = new WeakMap<ElectronApplication, string[]>();

let electronApplication: ElectronApplication | undefined;
let desktopEnvironment: Record<string, string> | undefined;
let hostProcess: ChildProcessWithoutNullStreams | undefined;
let rendererServer: Server | undefined;
let rendererUrl: string | undefined;
let runtimeDirectory: string | undefined;

test.beforeAll(async () => {
	const started = await startWebRendererServer();
	rendererServer = started.server;
	rendererUrl = started.url;
});

test.afterAll(async () => {
	const server = rendererServer;
	if (server !== undefined) {
		await new Promise<void>((resolve, reject) => {
			server.close((error) => (error === undefined ? resolve() : reject(error)));
		});
	}
	rendererServer = undefined;
	rendererUrl = undefined;
});

test.beforeEach(async () => {
	const temporaryRoot = process.platform === 'win32' ? tmpdir() : '/tmp';
	runtimeDirectory = await mkdtemp(path.join(temporaryRoot, 'orchbee-desktop-system-'));
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
		ORCHBEE_DESKTOP_ENDPOINT_ID: endpointId,
		ORCHBEE_DESKTOP_GATEWAY: '1',
		ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_RENDERER_URL: currentRendererUrl,
		ORCHBEE_DESKTOP_TRACE: '1',
		ORCHBEE_TERMINAL_HOST_BINARY: hostBinary,
		ORCHBEE_GATEWAY_ENTRY_PATH: gatewayMain,
		SHELL: process.platform === 'win32' ? (environment['ComSpec'] ?? 'cmd.exe') : '/bin/sh',
	};
	electronApplication = await launchDesktop();
});

test.afterEach(async () => {
	const runningApplication = electronApplication;
	if (runningApplication !== undefined) {
		try {
			await quitDesktop(runningApplication, await runningApplication.firstWindow(), 'preserve');
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

test('routes a real sandboxed Host terminal through Main and Preload to the renderer', async () => {
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const traceLines: string[] = [];
	application.process().stdout?.on('data', (chunk: Buffer) => traceLines.push(chunk.toString()));
	const page = await application.firstWindow();
	expect(new URL(page.url()).searchParams.get('orchbee-runtime')).toBe('desktop');
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
	const rendererAuthority = await page.evaluate(() => ({
		hasGenericOrchbeeApi: 'orchbee' in globalThis,
		hasNodeRequire: 'require' in globalThis,
	}));
	expect(rendererAuthority).toEqual({ hasGenericOrchbeeApi: false, hasNodeRequire: false });

	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.setSize(1_200, 650);
	});
	const input = page.getByRole('textbox');
	await input.pressSequentially(
		`i=1; while [ "$i" -le 160 ]; do printf 'history-%03d searchable-agent\\n' "$i"; i=$((i+1)); done; printf 'interaction-%s-marker allowed=https://orchbee.dev blocked=file:///tmp/orchbee-blocked\\n' tail`,
		{ delay: 1 },
	);
	await input.press('Enter');
	const accessibility = page.locator('[role="log"]');
	await expect(accessibility).toContainText('interaction-tail-marker');
	await expect(accessibility).toHaveAttribute('aria-live', 'polite');
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
	await expect(accessibility).toHaveAttribute('aria-live', 'off');
	await expect
		.poll(async () =>
			BigInt((await accessibleRows.first().getAttribute('data-terminal-absolute-row')) ?? '0'),
		)
		.toBeLessThan(liveFirstRow);

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await input.focus();
	await page.keyboard.press(`${primaryModifier}+KeyF`);
	const findSurface = page.getByRole('search');
	await expect(findSurface).toBeAttached();
	const findInput = findSurface.getByRole('textbox');
	await findInput.fill('history-001');
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
	await expect(accessibility).toHaveAttribute('aria-live', 'polite');
	await expect(accessibility).toContainText('interaction-tail-marker');

	const markerStart = await terminalCellCenter(
		root,
		canvas,
		accessibleRows,
		'interaction-tail-marker',
		0,
	);
	const markerEnd = await terminalCellCenter(
		root,
		canvas,
		accessibleRows,
		'interaction-tail-marker',
		10,
	);
	await page.mouse.move(markerStart.x, markerStart.y);
	await page.mouse.down();
	await page.mouse.move(markerEnd.x, markerEnd.y);
	await page.mouse.up();
	await expect(root).toHaveAttribute('data-projected-terminal-selection-state', 'active');

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
	const allowedLink = await terminalCellCenter(
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

	const blockedLink = await terminalCellCenter(
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
	await expect(page.locator('[role="log"]')).toContainText('interaction-tail-marker');

	await input.focus();
	await input.pressSequentially('stty size; exit', { delay: 1 });
	await input.press('Enter');
	await expect(root).toHaveAttribute('data-projected-terminal-state', 'exited');
	await expect(root).toHaveAttribute('data-projected-terminal-final-frame-presented', 'true');
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-committed-frames')))
		.toBeGreaterThan(0);
	await expect
		.poll(async () => Number(await root.getAttribute('data-projected-terminal-rendered-frames')))
		.toBeGreaterThan(0);
	await expect(root).toHaveAttribute(
		'data-projected-terminal-renderer-health',
		/^(?:fallback|ready|recovering|lost|recovery-failed)$/u,
	);
	await expect.poll(() => traceLines.join('').includes('orchbee.desktop.copy-path')).toBe(true);
	const traceText = traceLines.join('');
	expect(traceText).toContain('"terminalFramesReceived":');
	expect(traceText).toContain('"framesForwarded":');
	const identityBeforeExitedRestart = await terminalDocumentIdentity(root);

	await quitDesktop(application, page, 'preserve');
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();

	const exitedRestart = await launchDesktop();
	electronApplication = exitedRestart;
	const exitedPage = await exitedRestart.firstWindow();
	const exitedRoot = projectedTerminal(exitedPage);
	await expect(exitedRoot).toHaveAttribute('data-projected-terminal-state', 'exited');
	await expect(exitedRoot).toHaveAttribute('data-projected-terminal-final-frame-presented', 'true');
	const identityAfterExitedRestart = await terminalDocumentIdentity(exitedRoot);
	expect(identityAfterExitedRestart).toMatchObject({
		creationId: identityBeforeExitedRestart.creationId,
		terminalId: identityBeforeExitedRestart.terminalId,
	});
	expect(identityAfterExitedRestart.attachmentId).not.toBe(
		identityBeforeExitedRestart.attachmentId,
	);
	await quitDesktop(exitedRestart, exitedPage, 'preserve');
	electronApplication = undefined;
});

test('shares one Workbench and Host terminal with two authenticated Browser clients', async () => {
	test.setTimeout(60_000);
	const application = electronApplication;
	if (application === undefined) throw new Error('Electron application did not start.');
	const currentRendererUrl = rendererUrl;
	if (currentRendererUrl === undefined) throw new Error('Web renderer server is unavailable.');
	const gatewayUrl = await waitForGatewayBrowserUrl(application);
	const desktopPage = await application.firstWindow();
	const desktopRoot = projectedTerminal(desktopPage);
	await expect(desktopRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const desktopIdentity = await terminalDocumentIdentity(desktopRoot);
	const sharedPresentationId = await desktopRoot.getAttribute('data-terminal-presentation-id');

	const browser = await chromium.launch();
	try {
		const context = await browser.newContext();
		await context.grantPermissions(['clipboard-read', 'clipboard-write'], {
			origin: new URL(currentRendererUrl).origin,
		});
		const firstBrowserPage = await context.newPage();
		const secondBrowserPage = await context.newPage();
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
					.split(/\r?\n/u)
					.filter((line) => !line.startsWith('ORCHBEE_BROWSER_CLIENT_URL='));
				throw new Error(
					`First Browser projection failed. Browser=${JSON.stringify(firstSockets.events)} Desktop=${safeDesktopOutput.join('\n')}`,
					{ cause: error },
				);
			});
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

		const firstInput = firstBrowserPage.getByRole('textbox');
		await firstInput.pressSequentially("printf 'gateway-shared-terminal-marker\\n'");
		await firstInput.press('Enter');
		await expect(firstBrowserPage.locator('[role="log"]')).toContainText(
			'gateway-shared-terminal-marker',
		);
		await expect(secondBrowserPage.locator('[role="log"]')).toContainText(
			'gateway-shared-terminal-marker',
		);
		await expect(desktopPage.locator('[role="log"]')).toContainText(
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
		await expect(desktopPage.locator('[role="log"]')).toContainText(
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
		await expect(replacementPage.locator('[role="log"]')).toContainText(
			'gateway-after-disconnect-marker',
		);
		await expect.poll(() => gatewaySocketCount(replacementSockets, '/v1/control')).toBe(1);
		await expect
			.poll(() => gatewaySocketCount(replacementSockets, '/v1/attachment'))
			.toBeGreaterThanOrEqual(1);

		const linkTarget = new URL('/gateway-link-target', currentRendererUrl).href;
		await secondInput.pressSequentially(`printf 'browser-copy-marker ${linkTarget}\\n'`);
		await secondInput.press('Enter');
		const secondLog = secondBrowserPage.locator('[role="log"]');
		await expect(secondLog).toContainText('browser-copy-marker');
		const secondCanvas = secondBrowserPage.locator('canvas');
		const secondRows = secondLog.locator(':scope > div');
		const selectionStart = await terminalCellCenter(
			secondRoot,
			secondCanvas,
			secondRows,
			'browser-copy-marker',
			0,
		);
		const selectionEnd = await terminalCellCenter(
			secondRoot,
			secondCanvas,
			secondRows,
			'browser-copy-marker',
			18,
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

		const link = await terminalCellCenter(secondRoot, secondCanvas, secondRows, linkTarget, 4);
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

test('restarts Electron Main while retaining the Host terminal identity and contents', async () => {
	const firstApplication = electronApplication;
	if (firstApplication === undefined) throw new Error('Electron application did not start.');
	const firstPage = await firstApplication.firstWindow();
	const firstRoot = projectedTerminal(firstPage);
	await expect(firstRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	const input = firstPage.getByRole('textbox');
	await input.pressSequentially("printf 'recovery-main-marker\\n'");
	await input.press('Enter');
	await expect(firstPage.locator('[role="log"]')).toContainText('recovery-main-marker');
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

	await quitDesktop(firstApplication, firstPage, 'preserve');
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();

	const restartedApplication = await launchDesktop();
	electronApplication = restartedApplication;
	const restartedPage = await restartedApplication.firstWindow();
	const restartedRoot = projectedTerminal(restartedPage);
	await expect(restartedRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	await expect(restartedPage.locator('[role="log"]')).toContainText('recovery-main-marker');
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
	await expect(firstPage.locator('[role="log"]')).toContainText('stop-policy-marker');

	await quitDesktop(firstApplication, firstPage, 'stop');
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

	await stoppedSurface.locator('[data-slot="projected-terminal-start-new-session"]').click();
	const restartedRoot = projectedTerminal(restartedPage);
	await expect(restartedRoot).toHaveAttribute('data-projected-terminal-state', 'open');
	expect(await restartedRoot.getAttribute('data-terminal-presentation-id')).not.toBe(
		stoppedPresentationId,
	);

	await quitDesktop(restartedApplication, restartedPage, 'preserve');
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

async function waitForGatewayBrowserUrl(application: ElectronApplication): Promise<string> {
	const output = applicationOutput.get(application);
	if (output === undefined) throw new Error('Desktop output capture is unavailable.');
	let browserUrl: string | null = null;
	await expect
		.poll(() => {
			const match = /ORCHBEE_BROWSER_CLIENT_URL=([^\r\n]+)/u.exec(output.join(''));
			browserUrl = match?.[1] ?? null;
			return browserUrl;
		})
		.not.toBeNull();
	if (browserUrl === null) throw new Error('Desktop did not publish a Browser Gateway URL.');
	return browserUrl;
}

async function startWebRendererServer(): Promise<{
	readonly server: Server;
	readonly url: string;
}> {
	const server = createServer((request, response) => {
		void serveWebRendererRequest(request.url, response);
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

async function serveWebRendererRequest(
	requestUrl: string | undefined,
	response: ServerResponse,
): Promise<void> {
	try {
		const pathname = new URL(requestUrl ?? '/', 'http://127.0.0.1').pathname;
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
		response.statusCode = 404;
		response.end();
	}
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
	const endpoint = path.join(directory, `terminal-host-${endpointId}.sock`);
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

async function quitDesktop(
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
	await exited;
}

async function forceCloseDesktop(application: ElectronApplication): Promise<void> {
	const exited = waitForElectronExit(application);
	application.process().kill('SIGKILL');
	await Promise.race([exited, wait(2_000).then(() => undefined)]);
}

function waitForElectronExit(application: ElectronApplication): Promise<void> {
	const process = application.process();
	if (process.exitCode !== null) return Promise.resolve();
	return new Promise((resolve) => process.once('exit', () => resolve()));
}

function projectedTerminal(page: Page): Locator {
	return page.locator('[data-terminal-presentation-id][data-projected-terminal-state]').first();
}

function workbenchTabs(page: Page): Locator {
	return page.locator('[data-slot="workbench-tab-bar"] [role="tab"]');
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

function gatewayFrameShape(payload: string | Buffer): string {
	if (typeof payload !== 'string') return `binary:${payload.byteLength}`;
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

async function terminalCellCenter(
	root: Locator,
	canvas: Locator,
	rows: Locator,
	text: string,
	columnOffset: number,
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
		x: bounds.x + (location.column + columnOffset + 0.5) * (bounds.width / columns),
		y: bounds.y + (location.row + 0.5) * (bounds.height / visibleRows),
	};
}
