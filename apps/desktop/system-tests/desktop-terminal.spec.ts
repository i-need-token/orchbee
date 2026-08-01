import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import { _electron as electron, expect, test, type ElectronApplication } from '@playwright/test';

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
	electronApplication = await electron.launch({
		args: [desktopMain],
		env: {
			...environment,
			ORCHBEE_DESKTOP_ENDPOINT_ID: endpointId,
			ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: runtimeDirectory,
			ORCHBEE_DESKTOP_TRACE: '1',
			ORCHBEE_TERMINAL_HOST_BINARY: hostBinary,
			SHELL: process.platform === 'win32' ? (environment['ComSpec'] ?? 'cmd.exe') : '/bin/sh',
		},
	});
});

test.afterEach(async () => {
	await electronApplication?.close().catch(() => undefined);
	electronApplication = undefined;
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
	const root = page.locator('#root');
	await expect(root).toHaveAttribute('data-desktop-terminal-state', 'open');

	await expect(root).toHaveAttribute('data-desktop-context-isolated', 'true');
	await expect(root).toHaveAttribute('data-desktop-sandboxed', 'true');
	const rendererAuthority = await page.evaluate(() => ({
		hasGenericOrchbeeApi: 'orchbee' in globalThis,
		hasNodeRequire: 'require' in globalThis,
	}));
	expect(rendererAuthority).toEqual({ hasGenericOrchbeeApi: false, hasNodeRequire: false });

	await application.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.setSize(900, 650);
	});
	const input = page.getByRole('textbox');
	await input.pressSequentially("printf 'slice5-output-marker\\n'; stty size; exit", {
		delay: 1,
	});
	await input.press('Enter');
	await expect(page.locator('[role="log"]')).toContainText('slice5-output-marker');
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

	await application.close();
	electronApplication = undefined;
	expect(hostProcess?.exitCode).toBeNull();
});
