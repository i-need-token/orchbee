import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { access, chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { createConnection, createServer, type AddressInfo } from 'node:net';
import { tmpdir, userInfo } from 'node:os';
import path from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

import { _electron as electron, expect, test, type ElectronApplication } from '@playwright/test';

const packagedExecutable = process.env['ORCHBEE_SSH_PROJECT_ENROLLMENT_PACKAGE_EXECUTABLE'];
const runtimeVersion = process.env['ORCHBEE_SSH_PROJECT_ENROLLMENT_RUNTIME_VERSION'];
const terminalHostVersion = process.env['ORCHBEE_SSH_PROJECT_ENROLLMENT_TERMINAL_HOST_VERSION'];
const clientKeyPassphrase = 'orchbee-system-test-passphrase';

test('ssh-project-enrollment/SU-01-signed-loopback-lifecycle', async () => {
	test.skip(
		packagedExecutable === undefined ||
			runtimeVersion === undefined ||
			terminalHostVersion === undefined,
		'Run through pnpm test:system:ssh-project-enrollment to supply a signed release package.',
	);
	test.skip(process.platform === 'win32', 'SSH Project enrollment is POSIX Desktop-only.');
	test.setTimeout(180_000);
	if (
		packagedExecutable === undefined ||
		runtimeVersion === undefined ||
		terminalHostVersion === undefined
	) {
		throw new Error('The signed SSH Project enrollment package is unavailable.');
	}
	await access(packagedExecutable);
	const fixtureRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-enrollment-system-'));
	const applicationOutput: string[] = [];
	let application: ElectronApplication | undefined;
	const applicationProcessTree = new Map<number, ProcessObservation>();
	let sshFixture: LoopbackSshFixture | undefined;
	const rememberApplicationProcessTree = async (): Promise<void> => {
		const applicationProcessId = application?.process().pid;
		if (applicationProcessId === undefined) return;
		for (const processObservation of await observeProcessTree(applicationProcessId)) {
			applicationProcessTree.set(processObservation.pid, processObservation);
		}
	};
	try {
		const fixture = await createLoopbackSshFixture(fixtureRoot);
		sshFixture = fixture;
		const desktopHome = path.join(fixtureRoot, 'desktop-home');
		const desktopTemporaryDirectory = path.join(fixtureRoot, 'desktop-tmp');
		await mkdir(desktopHome, { mode: 0o700 });
		await mkdir(desktopTemporaryDirectory, { mode: 0o700 });
		const desktopSshDirectory = path.join(desktopHome, '.ssh');
		await mkdir(desktopSshDirectory, { mode: 0o700 });
		await writeFile(
			path.join(desktopSshDirectory, 'config'),
			await readFile(fixture.clientConfiguration),
			{ mode: 0o600 },
		);
		application = await electron.launch({
			args: [`--user-data-dir=${path.join(fixtureRoot, 'electron-user-data')}`],
			env: {
				...process.env,
				...(process.platform === 'darwin' ? { CFFIXED_USER_HOME: desktopHome } : {}),
				HOME: desktopHome,
				LANG: 'en_US.UTF-8',
				LC_ALL: 'en_US.UTF-8',
				ORCHBEE_DESKTOP_GATEWAY: '0',
				ORCHBEE_DESKTOP_TRACE: '1',
				SHELL: '/bin/sh',
				TMPDIR: desktopTemporaryDirectory,
			},
			executablePath: packagedExecutable,
		});
		application
			.process()
			.stdout?.on('data', (chunk: Buffer) => applicationOutput.push(chunk.toString()));
		application
			.process()
			.stderr?.on('data', (chunk: Buffer) => applicationOutput.push(chunk.toString()));
		const page = await application.firstWindow();
		page.setDefaultTimeout(30_000);
		await expect(page.locator('[data-slot="desktop-workbench"]')).toBeVisible();
		await rememberApplicationProcessTree();

		await page.locator('[data-slot="workbench-sidebar-project-create"] button').click();
		await page.getByRole('menuitem', { name: /^(?:Connect via SSH|通过 SSH 连接)…$/u }).click();
		const enrollment = page.locator('[data-slot="desktop-ssh-project-enrollment"]');
		await expect(enrollment).toBeVisible();
		await enrollment.locator('#ssh-project-destination').fill(fixture.destination);
		await enrollment.locator('#ssh-project-destination').press('Enter');

		await expect(enrollment.locator('pre')).toContainText(/authenticity|真实性/u);
		await enrollment.getByRole('button', { name: /^(?:Cancel|取消)$/u }).click();
		await expect(enrollment.locator('#ssh-project-destination')).toBeVisible();
		await expect(enrollment.locator('#ssh-project-destination')).toHaveValue(fixture.destination);
		await enrollment.getByRole('button', { name: /^(?:Cancel|取消)$/u }).click();
		await expect(enrollment).not.toBeVisible();
		await page.locator('[data-slot="workbench-sidebar-project-create"] button').click();
		await page.getByRole('menuitem', { name: /^(?:Connect via SSH|通过 SSH 连接)…$/u }).click();
		await expect(enrollment).toBeVisible();
		await enrollment.locator('#ssh-project-destination').fill(fixture.destination);
		await enrollment.locator('#ssh-project-destination').press('Enter');
		await expect(enrollment.locator('pre')).toContainText(/authenticity|真实性/u);
		const promptResponse = enrollment.locator('input[type="password"]');
		if ((await promptResponse.count()) > 0) await promptResponse.fill('yes');
		await enrollment.getByRole('button', { name: /^(?:Trust and Continue|信任并继续)$/u }).click();
		await expect(enrollment.locator('code')).toContainText(/passphrase|密码短语/u);
		const secretResponse = enrollment.locator('input[type="password"]');
		await expect(secretResponse).toBeVisible();
		await secretResponse.fill(clientKeyPassphrase);
		await enrollment.getByRole('button', { name: /^(?:Authenticate|认证)$/u }).click();
		const directoryBrowser = enrollment.locator('[data-slot="remote-directory-browser"]');
		await expect(directoryBrowser).toBeVisible({ timeout: 60_000 });
		await directoryBrowser.getByRole('button', { name: 'ssh-project', exact: true }).click();
		await expect(directoryBrowser.locator('[aria-current="page"]')).toHaveText('ssh-project');
		await directoryBrowser
			.getByRole('button', { name: /^(?:Use This Folder|使用此文件夹)$/u })
			.click();
		await expect(enrollment).not.toBeVisible({ timeout: 60_000 });

		const project = page.locator('li[data-project-id]').filter({ hasText: 'ssh-project' });
		await expect(project).toHaveCount(1);
		const projectStatus = project.locator(
			'[data-slot="workbench-sidebar-project"] span[data-state]',
		);
		const projectOptions = project.locator(
			'button[aria-label^="Project options:"], button[aria-label^="项目选项:"]',
		);
		const projectRow = project.locator('[data-project-reorder-row="true"]');
		await expect(projectStatus).toHaveAttribute('data-state', 'connected');
		await expect(page.locator('[data-terminal-presentation-id]')).toHaveCount(0);
		await expect(page.getByRole('button', { name: /^(?:New tab|新建标签页)$/u })).toBeEnabled();

		await assertSignedCohortInstalled(fixture.remoteHome, runtimeVersion, terminalHostVersion);
		await expect
			.poll(async () => (await readFile(fixture.knownHosts, 'utf8')).length)
			.toBeGreaterThan(0);
		const userDataPath = await application.evaluate(({ app }) => app.getPath('userData'));
		const workbenchDocumentPath = path.join(
			userDataPath,
			'terminal-recovery',
			'workbench-shared-document-v2.json',
		);
		const durableState = await Promise.all([
			readFile(path.join(userDataPath, 'environment-registry-v2.json'), 'utf8'),
			readFile(workbenchDocumentPath, 'utf8'),
		]);
		expect(durableState.join('\n')).toContain(fixture.destination);
		expect(durableState.join('\n')).toContain(fixture.projectPath);
		expect(durableState.join('\n')).not.toContain(clientKeyPassphrase);

		await projectRow.hover();
		await projectOptions.click();
		await page.getByRole('menuitem', { name: /^(?:Manage Project|管理项目)…$/u }).click();
		const projectManager = page.locator('[data-slot="workbench-project-manager"]');
		await expect(projectManager).toBeVisible();
		await expect(projectManager.getByLabel(/^(?:SSH remote folder|SSH 远程文件夹)$/u)).toHaveCount(
			1,
		);
		await projectManager.getByRole('button', { name: /^(?:Add Folder|添加文件夹)$/u }).click();
		await page.getByRole('menuitem', { name: /^(?:Add SSH Folder|添加 SSH 文件夹)…$/u }).click();
		await expect(enrollment).toBeVisible();
		await enrollment.locator('#ssh-project-destination').fill(fixture.destination);
		await enrollment.locator('#ssh-project-destination').press('Enter');
		await expect(directoryBrowser).toBeVisible({ timeout: 60_000 });
		await directoryBrowser.getByRole('button', { name: 'ssh-folder', exact: true }).click();
		await expect(directoryBrowser.locator('[aria-current="page"]')).toHaveText('ssh-folder');
		await directoryBrowser
			.getByRole('button', { name: /^(?:Use This Folder|使用此文件夹)$/u })
			.click();
		await expect(enrollment).not.toBeVisible({ timeout: 60_000 });
		await expect(projectManager).toBeVisible();
		await expect(projectManager.getByText('ssh-folder', { exact: true })).toBeVisible();
		await expect(projectManager.getByLabel(/^(?:SSH remote folder|SSH 远程文件夹)$/u)).toHaveCount(
			2,
		);
		expect(await readFile(workbenchDocumentPath, 'utf8')).toContain(fixture.attachmentPath);
		await projectManager.getByRole('button', { name: /^(?:Cancel|取消)$/u }).click();

		await page.getByRole('button', { name: /^(?:New tab|新建标签页)$/u }).click();
		const terminal = page.locator('[data-terminal-presentation-id]').first();
		await expect(terminal.locator('[data-terminal-projection-state]')).toHaveAttribute(
			'data-projected-terminal-state',
			'open',
			{ timeout: 60_000 },
		);
		const input = terminal.getByRole('textbox');
		await input.pressSequentially("printf 'orchbee-ssh-enrollment-io '; pwd", { delay: 1 });
		await input.press('Enter');
		await expect(terminal.locator('[role="list"]')).toContainText('orchbee-ssh-enrollment-io');
		await expect(terminal.locator('[role="list"]')).toContainText(fixture.projectPath);

		await fixture.stopServer();
		await expect
			.poll(() => projectStatus.getAttribute('data-state'), { timeout: 30_000 })
			.toMatch(/^(?:blocked|retrying)$/u);
		await fixture.startServer();
		await expect(projectStatus).toHaveAttribute('data-state', 'blocked', { timeout: 30_000 });
		await projectRow.hover();
		await projectOptions.click();
		await page.getByRole('menuitem', { name: /^(?:Retry Connection|重试连接)$/u }).click();
		await expect(enrollment.locator('code')).toContainText(/passphrase|密码短语/u);
		await expect(secretResponse).toBeVisible();
		await secretResponse.fill(clientKeyPassphrase);
		await enrollment.getByRole('button', { name: /^(?:Authenticate|认证)$/u }).click();
		try {
			await expect(enrollment).not.toBeVisible({ timeout: 30_000 });
		} catch (error: unknown) {
			throw new Error(`Retry dialog remained open: ${await enrollment.innerText()}`, {
				cause: error,
			});
		}
		await expect(projectStatus).toHaveAttribute('data-state', 'connected');

		await projectRow.hover();
		await projectOptions.click();
		await page.getByRole('menuitem', { name: /^(?:Disconnect|断开连接)$/u }).click();
		await expect(projectStatus).toHaveAttribute('data-state', 'disconnected');
		expect(await readFile(workbenchDocumentPath, 'utf8')).toContain(fixture.projectPath);
	} catch (error: unknown) {
		const output = applicationOutput.join('').slice(-32 * 1024);
		const enrollmentText =
			(await application?.firstWindow().then((page) =>
				page
					.locator('[data-slot="desktop-ssh-project-enrollment"]')
					.innerText()
					.catch(() => ''),
			)) ?? '';
		const fixtureProcesses = (await observeProcesses())
			.filter((processObservation) => processObservation.command.includes(fixtureRoot))
			.map(
				(processObservation) =>
					`${processObservation.pid} ${processObservation.parentPid} ${processObservation.command}`,
			)
			.join('\n');
		throw new Error(
			[
				'Signed loopback SSH Project enrollment failed.',
				enrollmentText,
				output,
				sshFixture?.diagnostics() ?? '',
				fixtureProcesses,
			]
				.filter((part) => part.length > 0)
				.join('\n'),
			{ cause: error },
		);
	} finally {
		try {
			await rememberApplicationProcessTree();
			await application?.close().catch(() => undefined);
			await stopObservedProcesses([...applicationProcessTree.values()]);
			await sshFixture?.dispose();
		} finally {
			await stopProcessesContaining(fixtureRoot);
			await rm(fixtureRoot, { force: true, recursive: true });
		}
	}
});

interface LoopbackSshFixture {
	readonly attachmentPath: string;
	readonly clientConfiguration: string;
	readonly destination: string;
	diagnostics(): string;
	dispose(): Promise<void>;
	readonly knownHosts: string;
	readonly projectPath: string;
	readonly remoteHome: string;
	startServer(): Promise<void>;
	stopServer(): Promise<void>;
}

async function createLoopbackSshFixture(root: string): Promise<LoopbackSshFixture> {
	const directory = path.join(root, 'loopback-sshd');
	const remoteHome = path.join(directory, 'remote-home');
	const projectPath = path.join(remoteHome, 'ssh-project');
	const attachmentPath = path.join(remoteHome, 'ssh-folder');
	await mkdir(projectPath, { mode: 0o700, recursive: true });
	await mkdir(attachmentPath, { mode: 0o700, recursive: true });
	const hostKey = path.join(directory, 'host-key');
	const clientKey = path.join(directory, 'client-key');
	await runProgram('/usr/bin/ssh-keygen', ['-q', '-t', 'ed25519', '-N', '', '-f', hostKey]);
	await runProgram('/usr/bin/ssh-keygen', [
		'-q',
		'-t',
		'ed25519',
		'-a',
		'4',
		'-N',
		clientKeyPassphrase,
		'-f',
		clientKey,
	]);
	const authorizedKeys = path.join(directory, 'authorized-keys');
	await writeFile(authorizedKeys, await readFile(`${clientKey}.pub`, 'utf8'), { mode: 0o600 });
	const forcedCommand = path.join(directory, 'forced-command.sh');
	await writeFile(
		forcedCommand,
		`#!/bin/sh\nexport HOME=${quotePosixShellWord(remoteHome)}\nexec /bin/sh -c "$SSH_ORIGINAL_COMMAND"\n`,
		{ mode: 0o700 },
	);
	await chmod(forcedCommand, 0o700);
	const port = await reserveLoopbackPort();
	const username = userInfo().username;
	const serverConfiguration = path.join(directory, 'sshd-config');
	await writeFile(
		serverConfiguration,
		[
			`Port ${port}`,
			'ListenAddress 127.0.0.1',
			`HostKey ${hostKey}`,
			`PidFile ${path.join(directory, 'sshd.pid')}`,
			`AuthorizedKeysFile ${authorizedKeys}`,
			'AuthenticationMethods publickey',
			'PubkeyAuthentication yes',
			'PasswordAuthentication no',
			'KbdInteractiveAuthentication no',
			'UsePAM no',
			'PermitRootLogin no',
			'StrictModes no',
			`AllowUsers ${username}`,
			'AllowTcpForwarding no',
			'X11Forwarding no',
			'PermitTunnel no',
			'PermitUserEnvironment no',
			'PrintMotd no',
			'PrintLastLog no',
			`ForceCommand ${forcedCommand}`,
			'LogLevel VERBOSE',
			'',
		].join('\n'),
		{ mode: 0o600 },
	);
	await runProgram('/usr/sbin/sshd', ['-t', '-f', serverConfiguration]);
	const destination = 'orchbee-loopback-enrollment';
	const knownHosts = path.join(directory, 'known-hosts');
	const clientConfiguration = path.join(directory, 'ssh-config');
	await writeFile(
		clientConfiguration,
		[
			`Host ${destination}`,
			'  HostName 127.0.0.1',
			`  HostKeyAlias ${destination}`,
			`  Port ${port}`,
			`  User ${username}`,
			`  IdentityFile ${clientKey}`,
			'  IdentitiesOnly yes',
			'  IdentityAgent none',
			'  PreferredAuthentications publickey',
			'  PasswordAuthentication no',
			'  KbdInteractiveAuthentication no',
			'  StrictHostKeyChecking ask',
			`  UserKnownHostsFile ${knownHosts}`,
			'  GlobalKnownHostsFile /dev/null',
			'  UpdateHostKeys no',
			'',
		].join('\n'),
		{ mode: 0o600 },
	);

	let server: ChildProcessWithoutNullStreams | undefined;
	let serverDiagnostics = '';
	const startServer = async (): Promise<void> => {
		if (server !== undefined) throw new Error('Loopback sshd is already running.');
		server = spawn('/usr/sbin/sshd', ['-D', '-e', '-f', serverConfiguration], {
			detached: true,
			stdio: ['pipe', 'pipe', 'pipe'],
		});
		server.stderr.setEncoding('utf8');
		server.stderr.on('data', (chunk: string) => {
			serverDiagnostics += chunk;
		});
		await expect
			.poll(
				async () => {
					if (server?.exitCode !== null) {
						throw new Error(`Loopback sshd exited before readiness: ${serverDiagnostics}`);
					}
					return canConnectToLoopbackPort(port);
				},
				{ timeout: 5_000 },
			)
			.toBe(true);
	};
	const stopServer = async (): Promise<void> => {
		const running = server;
		server = undefined;
		if (running === undefined || running.pid === undefined || running.exitCode !== null) return;
		await stopSshdSessionProcesses(running.pid);
		await stopFixtureAttachmentProcesses(remoteHome);
		try {
			process.kill(-running.pid, 'SIGTERM');
		} catch {
			running.kill('SIGTERM');
		}
		await Promise.race([
			new Promise<void>((resolve) => running.once('exit', () => resolve())),
			wait(5_000).then(() => undefined),
		]);
		if (running.exitCode === null) {
			try {
				process.kill(-running.pid, 'SIGKILL');
			} catch {
				running.kill('SIGKILL');
			}
		}
		await expect.poll(() => canConnectToLoopbackPort(port)).toBe(false);
	};
	await startServer();
	return {
		attachmentPath,
		clientConfiguration,
		destination,
		diagnostics: () => serverDiagnostics,
		dispose: async () => {
			await stopServer();
			await stopFixtureProcesses(remoteHome);
		},
		knownHosts,
		projectPath,
		remoteHome,
		startServer,
		stopServer,
	};
}

async function assertSignedCohortInstalled(
	remoteHome: string,
	runtimeReleaseVersion: string,
	terminalHostReleaseVersion: string,
): Promise<void> {
	const artifacts = [
		path.join(
			remoteHome,
			'.orchbee',
			'ssh-runtime',
			'versions',
			runtimeReleaseVersion,
			'orchbee-ssh-environment-runtime',
		),
		path.join(
			remoteHome,
			'.orchbee',
			'terminal-host',
			'versions',
			terminalHostReleaseVersion,
			'orchbee-terminal-host',
		),
	];
	const metadata = await Promise.all(artifacts.map(async (artifact) => stat(artifact)));
	for (const artifactMetadata of metadata) {
		expect(artifactMetadata.isFile()).toBe(true);
		expect(artifactMetadata.size).toBeGreaterThan(0);
		expect(artifactMetadata.mode & 0o111).not.toBe(0);
	}
}

async function reserveLoopbackPort(): Promise<number> {
	const server = createServer();
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', () => resolve());
	});
	const address = server.address() as AddressInfo;
	await new Promise<void>((resolve, reject) => {
		server.close((error) => (error === undefined ? resolve() : reject(error)));
	});
	return address.port;
}

function canConnectToLoopbackPort(port: number): Promise<boolean> {
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
		throw new Error(`${executable} ${arguments_.join(' ')} failed with ${exitCode}: ${output}`);
	}
}

async function stopFixtureProcesses(remoteHome: string): Promise<void> {
	const ownedPrefix = path.join(remoteHome, '.orchbee');
	await stopProcessesContaining(ownedPrefix);
}

async function stopProcessesContaining(commandFragment: string): Promise<void> {
	const ownedProcesses = (await observeProcesses()).filter((processObservation) =>
		processObservation.command.includes(commandFragment),
	);
	await stopObservedProcesses(ownedProcesses);
}

async function stopObservedProcesses(ownedProcesses: readonly ProcessObservation[]): Promise<void> {
	if (ownedProcesses.length === 0) return;
	const ownedByProcessId = new Map(
		ownedProcesses.map((processObservation) => [
			processObservation.pid,
			processObservation.command,
		]),
	);
	const matchingProcesses = async (): Promise<readonly ProcessObservation[]> =>
		(await observeProcesses()).filter(
			(processObservation) =>
				ownedByProcessId.get(processObservation.pid) === processObservation.command,
		);
	const signalProcesses = async (signal: NodeJS.Signals): Promise<void> => {
		for (const processObservation of await matchingProcesses()) {
			try {
				process.kill(processObservation.pid, signal);
			} catch {
				// A process that already exited owns no remaining fixture resource.
			}
		}
	};
	const hasProcesses = async (): Promise<boolean> => (await matchingProcesses()).length > 0;
	await signalProcesses('SIGTERM');
	try {
		await expect.poll(hasProcesses, { timeout: 5_000 }).toBe(false);
		return;
	} catch {
		await signalProcesses('SIGKILL');
	}
	await expect.poll(hasProcesses, { timeout: 5_000 }).toBe(false);
}

async function observeProcessTree(rootProcessId: number): Promise<readonly ProcessObservation[]> {
	const processes = await observeProcesses();
	const processIds = new Set([rootProcessId]);
	let changed = true;
	while (changed) {
		changed = false;
		for (const processObservation of processes) {
			if (!processIds.has(processObservation.pid) && processIds.has(processObservation.parentPid)) {
				processIds.add(processObservation.pid);
				changed = true;
			}
		}
	}
	return processes.filter((processObservation) => processIds.has(processObservation.pid));
}

async function stopSshdSessionProcesses(listenerPid: number): Promise<void> {
	const processes = await observeProcesses();
	const descendantPids = new Set([listenerPid]);
	let changed = true;
	while (changed) {
		changed = false;
		for (const processObservation of processes) {
			if (
				!descendantPids.has(processObservation.pid) &&
				descendantPids.has(processObservation.parentPid)
			) {
				descendantPids.add(processObservation.pid);
				changed = true;
			}
		}
	}
	for (const processObservation of [...processes].reverse()) {
		if (
			processObservation.pid === listenerPid ||
			!descendantPids.has(processObservation.pid) ||
			!processObservation.command.includes('sshd')
		) {
			continue;
		}
		try {
			process.kill(processObservation.pid, 'SIGKILL');
		} catch {
			// A session process that already exited has already severed its connection.
		}
	}
}

async function stopFixtureAttachmentProcesses(remoteHome: string): Promise<void> {
	const runtimePrefix = path.join(remoteHome, '.orchbee', 'ssh-runtime');
	const processes = await observeProcesses();
	for (const processObservation of processes) {
		if (
			!processObservation.command.includes(runtimePrefix) ||
			!processObservation.command.includes(' attach ')
		) {
			continue;
		}
		try {
			process.kill(processObservation.pid, 'SIGKILL');
		} catch {
			// An attachment that already exited has already released remote ownership.
		}
	}
	await expect
		.poll(async () =>
			(await observeProcesses()).some(
				(processObservation) =>
					processObservation.command.includes(runtimePrefix) &&
					processObservation.command.includes(' attach '),
			),
		)
		.toBe(false);
}

interface ProcessObservation {
	readonly command: string;
	readonly parentPid: number;
	readonly pid: number;
}

async function observeProcesses(): Promise<readonly ProcessObservation[]> {
	const child = spawn('/bin/ps', ['-axo', 'pid=,ppid=,command='], {
		stdio: ['ignore', 'pipe', 'pipe'],
	});
	let output = '';
	child.stdout.setEncoding('utf8');
	child.stdout.on('data', (chunk: string) => {
		output += chunk;
	});
	await new Promise<void>((resolve) => child.once('exit', () => resolve()));
	return output
		.split('\n')
		.map((line) => /^\s*([0-9]+)\s+([0-9]+)\s+(.*)$/u.exec(line))
		.filter((match): match is RegExpExecArray => match !== null)
		.map((match) => ({
			command: match[3] ?? '',
			parentPid: Number(match[2]),
			pid: Number(match[1]),
		}));
}

function quotePosixShellWord(value: string): string {
	return `'${value.replaceAll("'", `'"'"'`)}'`;
}
