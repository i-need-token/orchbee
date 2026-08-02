import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import {
	_electron as electron,
	expect,
	test,
	type ElectronApplication,
	type Locator,
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

let electronApplication: ElectronApplication | undefined;
let desktopEnvironment: Record<string, string> | undefined;
let hostProcess: ChildProcessWithoutNullStreams | undefined;
let runtimeDirectory: string | undefined;

test.beforeEach(async () => {
	const temporaryRoot = process.platform === 'win32' ? tmpdir() : '/tmp';
	runtimeDirectory = await mkdtemp(path.join(temporaryRoot, 'orchbee-desktop-system-'));
	const endpointId = randomBytes(16).toString('hex');
	hostProcess = spawn(
		hostBinary,
		['serve', '--runtime-directory', runtimeDirectory, '--endpoint-id', endpointId],
		{ stdio: ['pipe', 'pipe', 'pipe'] },
	);
	if (process.platform === 'win32') {
		await wait(250);
	} else {
		const endpoint = path.join(runtimeDirectory, `terminal-host-${endpointId}.sock`);
		await expect
			.poll(
				async () =>
					access(endpoint)
						.then(() => true)
						.catch(() => false),
				{ timeout: 5_000 },
			)
			.toBe(true);
	}

	const environment = Object.fromEntries(
		Object.entries(process.env).filter(
			(entry): entry is [string, string] => entry[1] !== undefined,
		),
	);
	desktopEnvironment = {
		...environment,
		ORCHBEE_DESKTOP_ENDPOINT_ID: endpointId,
		ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: runtimeDirectory,
		ORCHBEE_DESKTOP_TRACE: '1',
		ORCHBEE_TERMINAL_HOST_BINARY: hostBinary,
		SHELL: process.platform === 'win32' ? (environment['ComSpec'] ?? 'cmd.exe') : '/bin/sh',
	};
	electronApplication = await launchDesktop();
});

test.afterEach(async () => {
	await electronApplication?.close().catch(() => undefined);
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
	let workerStarts = 0;
	page.on('worker', () => {
		workerStarts += 1;
	});
	const applicationRoot = page.locator('#root');
	const root = projectedTerminal(page);
	await expect(root).toHaveAttribute('data-desktop-terminal-state', 'open');

	await expect(applicationRoot).toHaveAttribute('data-desktop-context-isolated', 'true');
	await expect(applicationRoot).toHaveAttribute('data-desktop-sandboxed', 'true');
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
		.toBe(Number(await root.getAttribute('data-desktop-terminal-rows')));
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
	await expect(root).toHaveAttribute('data-desktop-terminal-selection-state', 'active');

	if (process.platform === 'darwin') {
		const gestureSelectionStart = await root.getAttribute('data-desktop-terminal-selection-start');
		await input.focus();
		await page.keyboard.press('Meta+KeyA');
		await expect
			.poll(() => root.getAttribute('data-desktop-terminal-selection-start'))
			.not.toBe(gestureSelectionStart);
	}
	await input.focus();
	await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyC' : 'Control+Shift+KeyC');
	await expect
		.poll(async () => Number(await root.getAttribute('data-desktop-terminal-copied-bytes')))
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
	await expect(root).toHaveAttribute('data-desktop-terminal-state', 'open');
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
	await expect(root).toHaveAttribute('data-desktop-terminal-state', 'exited');
	await expect(root).toHaveAttribute('data-desktop-terminal-final-frame-presented', 'true');
	await expect
		.poll(async () => Number(await root.getAttribute('data-desktop-terminal-committed-frames')))
		.toBeGreaterThan(0);
	await expect
		.poll(async () => Number(await root.getAttribute('data-desktop-terminal-rendered-frames')))
		.toBeGreaterThan(0);
	await expect(root).toHaveAttribute(
		'data-desktop-terminal-renderer-health',
		/^(?:fallback|ready|recovering|lost|recovery-failed)$/u,
	);
	await expect.poll(() => traceLines.join('').includes('orchbee.desktop.copy-path')).toBe(true);
	const traceText = traceLines.join('');
	expect(traceText).toContain('"terminalFramesReceived":');
	expect(traceText).toContain('"framesForwarded":');
	const identityBeforeExitedRestart = await terminalDocumentIdentity(root);

	await application.close();
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();

	const exitedRestart = await launchDesktop();
	electronApplication = exitedRestart;
	const exitedPage = await exitedRestart.firstWindow();
	const exitedRoot = projectedTerminal(exitedPage);
	await expect(exitedRoot).toHaveAttribute('data-desktop-terminal-state', 'exited');
	await expect(exitedRoot).toHaveAttribute('data-desktop-terminal-final-frame-presented', 'true');
	const identityAfterExitedRestart = await terminalDocumentIdentity(exitedRoot);
	expect(identityAfterExitedRestart).toMatchObject({
		creationId: identityBeforeExitedRestart.creationId,
		terminalId: identityBeforeExitedRestart.terminalId,
	});
	expect(identityAfterExitedRestart.attachmentId).not.toBe(
		identityBeforeExitedRestart.attachmentId,
	);
	await exitedRestart.close();
	electronApplication = undefined;
});

test('restarts Electron Main while retaining the Host terminal identity and contents', async () => {
	const firstApplication = electronApplication;
	if (firstApplication === undefined) throw new Error('Electron application did not start.');
	const firstPage = await firstApplication.firstWindow();
	const firstRoot = projectedTerminal(firstPage);
	await expect(firstRoot).toHaveAttribute('data-desktop-terminal-state', 'open');
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

	await firstApplication.close();
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();

	const restartedApplication = await launchDesktop();
	electronApplication = restartedApplication;
	const restartedPage = await restartedApplication.firstWindow();
	const restartedRoot = projectedTerminal(restartedPage);
	await expect(restartedRoot).toHaveAttribute('data-desktop-terminal-state', 'open');
	await expect(restartedPage.locator('[role="log"]')).toContainText('recovery-main-marker');
	const identityAfterRestart = await terminalDocumentIdentity(restartedRoot);
	expect(identityAfterRestart).toMatchObject({
		creationId: identityBeforeRestart.creationId,
		terminalId: identityBeforeRestart.terminalId,
	});
	expect(identityAfterRestart.attachmentId).not.toBe(identityBeforeRestart.attachmentId);
	expect(identityAfterRestart.hostEpoch).toBe(identityBeforeRestart.hostEpoch);
});

async function launchDesktop(): Promise<ElectronApplication> {
	if (desktopEnvironment === undefined) throw new Error('Desktop test environment is unavailable.');
	return electron.launch({ args: [desktopMain], env: desktopEnvironment });
}

function projectedTerminal(page: Awaited<ReturnType<ElectronApplication['firstWindow']>>): Locator {
	return page.locator('[data-terminal-presentation-id][data-desktop-terminal-state]').first();
}

async function terminalDocumentIdentity(root: Locator): Promise<{
	readonly attachmentId: string | null;
	readonly creationId: string | null;
	readonly hostEpoch: string | null;
	readonly terminalId: string | null;
}> {
	return {
		attachmentId: await root.getAttribute('data-desktop-terminal-attachment-id'),
		creationId: await root.getAttribute('data-desktop-terminal-creation-id'),
		hostEpoch: await root.getAttribute('data-desktop-terminal-host-epoch'),
		terminalId: await root.getAttribute('data-desktop-terminal-id'),
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
	const columns = Number(await root.getAttribute('data-desktop-terminal-columns'));
	const visibleRows = Number(await root.getAttribute('data-desktop-terminal-rows'));
	return {
		x: bounds.x + (location.column + columnOffset + 0.5) * (bounds.width / columns),
		y: bounds.y + (location.row + 0.5) * (bounds.height / visibleRows),
	};
}
