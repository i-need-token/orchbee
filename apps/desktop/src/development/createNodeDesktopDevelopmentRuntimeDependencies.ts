import { spawn, type ChildProcess } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { watch, type FSWatcher } from 'node:fs';
import { chmod, copyFile, mkdir, mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { TerminalConfigurationSchema } from '@orchbee/terminal-protocol';

import type { DesktopDevelopmentRestartRequest } from './desktop-development-contract';
import { createInteractiveTerminalHostEnvironment } from './createInteractiveTerminalHostEnvironment';
import { DesktopDevelopmentRestartPlanStore } from './DesktopDevelopmentRestartPlanStore';
import { observeTerminalHostBuild } from './observeTerminalHostBuild';
import type {
	DesktopDevelopmentHostBuild,
	DesktopDevelopmentHostBuildWatch,
	DesktopDevelopmentHostCandidate,
	DesktopDevelopmentRuntimeDependencies,
	DesktopDevelopmentRuntimeLogger,
	DesktopDevelopmentSupervisedProcess,
	DesktopDevelopmentWorkspace,
} from './DesktopDevelopmentRuntime';

const ENDPOINT_ID_PATTERN = /^[0-9a-f]{32}$/u;
const HOST_BUILD_DEBOUNCE_MILLISECONDS = 100;
const PROCESS_TERMINATION_TIMEOUT_MILLISECONDS = 2_000;

/** @internal Development-tool adapter consumed only by the Desktop Rsbuild configuration. */
export function createNodeDesktopDevelopmentRuntimeDependencies(
	logger: DesktopDevelopmentRuntimeLogger,
): DesktopDevelopmentRuntimeDependencies {
	const candidateDirectories = new WeakMap<DesktopDevelopmentHostCandidate, string>();
	const workspaceRoots = new WeakMap<DesktopDevelopmentWorkspace, string>();

	return {
		buildInitialHost: async (terminalHostRoot) => {
			await runCommand('zig', ['build', '-fincremental', '-Doptimize=ReleaseSafe'], {
				cwd: terminalHostRoot,
			});
			const binaryPath = terminalHostBinaryPath(terminalHostRoot);
			return {
				binaryPath,
				digest: await fileDigest(binaryPath),
				inputDigest: await terminalHostInputDigest(terminalHostRoot),
			};
		},
		captureHostCandidate: async (build, workspace, generation) => {
			const workspaceRoot = workspaceRoots.get(workspace);
			if (workspaceRoot === undefined) {
				throw new Error('Desktop development workspace root is unavailable.');
			}
			const directory = path.join(workspaceRoot, 'host-generations', String(generation));
			await mkdir(directory, { mode: 0o700, recursive: true });
			const binaryPath = path.join(directory, path.basename(build.binaryPath));
			try {
				await copyFile(build.binaryPath, binaryPath);
				if (process.platform !== 'win32') await chmod(binaryPath, 0o700);
				const digest = await fileDigest(binaryPath);
				if (digest !== build.digest) {
					throw new Error('Terminal Host build changed while its generation was being captured.');
				}
				const candidate = { binaryPath, digest, generation, inputDigest: build.inputDigest };
				candidateDirectories.set(candidate, directory);
				return candidate;
			} catch (error: unknown) {
				await rm(directory, { force: true, recursive: true }).catch(() => undefined);
				throw error;
			}
		},
		cleanupWorkspace: async (workspace) => {
			const workspaceRoot = workspaceRoots.get(workspace);
			if (workspaceRoot === undefined) return;
			workspaceRoots.delete(workspace);
			await rm(workspaceRoot, { force: true, recursive: true });
		},
		createRestartPlan: (filePath) => new DesktopDevelopmentRestartPlanStore(filePath),
		createWorkspace: async (environment) => {
			const endpointId =
				environment['ORCHBEE_DESKTOP_ENDPOINT_ID'] ?? randomBytes(16).toString('hex');
			if (!ENDPOINT_ID_PATTERN.test(endpointId)) {
				throw new Error('ORCHBEE_DESKTOP_ENDPOINT_ID must be 32 lowercase hex digits.');
			}
			const terminalScrollbackBytes = resolveScrollbackBytes(
				environment['ORCHBEE_TERMINAL_SCROLLBACK_BYTES'],
			);
			const workspaceRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-development-'));
			const runtimeDirectory = resolveOptionalDirectory(
				environment['ORCHBEE_DESKTOP_RUNTIME_DIRECTORY'],
				path.join(workspaceRoot, 'runtime'),
			);
			const recoveryDirectory = resolveOptionalDirectory(
				environment['ORCHBEE_DESKTOP_RECOVERY_DIRECTORY'],
				path.join(workspaceRoot, 'recovery'),
			);
			await Promise.all([
				mkdir(runtimeDirectory, { mode: 0o700, recursive: true }),
				mkdir(recoveryDirectory, { mode: 0o700, recursive: true }),
			]);
			const workspace: DesktopDevelopmentWorkspace = {
				endpointId,
				recoveryDirectory,
				restartPlanPath: path.join(workspaceRoot, 'restart-plan-v1.json'),
				runtimeDirectory,
				terminalScrollbackBytes,
			};
			workspaceRoots.set(workspace, workspaceRoot);
			return workspace;
		},
		releaseHostCandidate: async (candidate) => {
			const directory = candidateDirectories.get(candidate);
			if (directory === undefined) return;
			candidateDirectories.delete(candidate);
			await rm(directory, { force: true, recursive: true });
		},
		spawnElectron: async (input) =>
			spawnSupervisedProcess(input.executable, [input.entryPath], {
				env: input.environment,
				stdio: ['inherit', 'inherit', 'inherit', 'ipc'],
			}),
		spawnHost: async ({ candidate, workspace }) => {
			const child = await spawnSupervisedProcess(
				candidate.binaryPath,
				[
					'serve',
					'--runtime-directory',
					workspace.runtimeDirectory,
					'--endpoint-id',
					workspace.endpointId,
					'--scrollback-bytes',
					workspace.terminalScrollbackBytes,
				],
				{ env: createInteractiveTerminalHostEnvironment(process.env), stdio: 'inherit' },
			);
			if (await child.waitForExit(HOST_BUILD_DEBOUNCE_MILLISECONDS)) {
				throw new Error('Terminal Host exited before accepting Desktop connections.');
			}
			return child;
		},
		watchHostBuilds: async (terminalHostRoot, initialBuild, onBuild) =>
			startHostBuildWatch(terminalHostRoot, initialBuild, onBuild, logger),
	};
}

class NodeSupervisedProcess implements DesktopDevelopmentSupervisedProcess {
	readonly #child: ChildProcess;

	constructor(child: ChildProcess) {
		this.#child = child;
		child.on('error', () => undefined);
	}

	get connected(): boolean {
		return this.#child.connected;
	}

	async send(message: DesktopDevelopmentRestartRequest): Promise<boolean> {
		if (!this.#child.connected || this.#child.send === undefined) return false;
		return new Promise((resolve) => {
			try {
				this.#child.send?.(message, (error) => resolve(error === null));
			} catch {
				resolve(false);
			}
		});
	}

	async terminate(): Promise<void> {
		if (hasExited(this.#child)) return;
		this.#child.kill();
		if (await this.waitForExit(PROCESS_TERMINATION_TIMEOUT_MILLISECONDS)) return;
		this.#child.kill('SIGKILL');
		await this.waitForExit(PROCESS_TERMINATION_TIMEOUT_MILLISECONDS);
	}

	waitForExit(timeoutMilliseconds: number): Promise<boolean> {
		if (hasExited(this.#child)) return Promise.resolve(true);
		if (timeoutMilliseconds <= 0) return Promise.resolve(false);
		return new Promise((resolve) => {
			let settled = false;
			const finish = (exited: boolean): void => {
				if (settled) return;
				settled = true;
				clearTimeout(timeout);
				this.#child.off('exit', handleExit);
				resolve(exited);
			};
			const handleExit = (): void => finish(true);
			const timeout = setTimeout(() => finish(false), timeoutMilliseconds);
			this.#child.once('exit', handleExit);
			if (hasExited(this.#child)) finish(true);
		});
	}
}

class NodeHostBuildWatch implements DesktopDevelopmentHostBuildWatch {
	readonly #buildProcess: DesktopDevelopmentSupervisedProcess;
	readonly #fileWatcher: FSWatcher;
	readonly #clearDebounce: () => void;
	readonly #drain: () => Promise<void>;

	constructor(
		buildProcess: DesktopDevelopmentSupervisedProcess,
		fileWatcher: FSWatcher,
		clearDebounce: () => void,
		drain: () => Promise<void>,
	) {
		this.#buildProcess = buildProcess;
		this.#fileWatcher = fileWatcher;
		this.#clearDebounce = clearDebounce;
		this.#drain = drain;
	}

	async close(): Promise<void> {
		this.#clearDebounce();
		let firstFailure: unknown;
		try {
			this.#fileWatcher.close();
		} catch (error: unknown) {
			firstFailure = error;
		}
		try {
			await this.#buildProcess.terminate();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#drain();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		if (firstFailure !== undefined) throw firstFailure;
	}
}

async function startHostBuildWatch(
	terminalHostRoot: string,
	initialBuild: DesktopDevelopmentHostBuild,
	onBuild: (build: DesktopDevelopmentHostBuild) => Promise<void>,
	logger: DesktopDevelopmentRuntimeLogger,
): Promise<DesktopDevelopmentHostBuildWatch> {
	const binaryPath = terminalHostBinaryPath(terminalHostRoot);
	let currentDigest = initialBuild.digest;
	let debounce: ReturnType<typeof setTimeout> | undefined;
	let operationTail = Promise.resolve();
	const inspectBuild = (): void => {
		operationTail = operationTail
			.then(async () => {
				currentDigest = await observeTerminalHostBuild({
					binaryPath,
					currentDigest,
					onBuild,
					readBinaryDigest: fileDigest,
					readInputDigest: () => terminalHostInputDigest(terminalHostRoot),
				});
			})
			.catch((error: unknown) => {
				logger.error(`Could not observe the Terminal Host build: ${errorMessage(error)}`);
			});
	};
	const fileWatcher = watch(path.dirname(binaryPath), (_eventType, filename) => {
		if (filename !== null && filename.toString() !== path.basename(binaryPath)) return;
		if (debounce !== undefined) clearTimeout(debounce);
		debounce = setTimeout(inspectBuild, HOST_BUILD_DEBOUNCE_MILLISECONDS);
	});
	fileWatcher.on('error', (error) => {
		logger.error(`Terminal Host binary watch failed: ${error.message}`);
	});
	try {
		const buildProcess = await spawnSupervisedProcess(
			'zig',
			['build', '--watch', '-fincremental', '-Doptimize=ReleaseSafe'],
			{ cwd: terminalHostRoot, env: process.env, stdio: 'inherit' },
		);
		return new NodeHostBuildWatch(
			buildProcess,
			fileWatcher,
			() => {
				if (debounce !== undefined) clearTimeout(debounce);
				debounce = undefined;
			},
			() => operationTail,
		);
	} catch (error: unknown) {
		fileWatcher.close();
		throw error;
	}
}

async function spawnSupervisedProcess(
	command: string,
	arguments_: readonly string[],
	options: Parameters<typeof spawn>[2],
): Promise<DesktopDevelopmentSupervisedProcess> {
	const child = spawn(command, [...arguments_], options);
	const process = new NodeSupervisedProcess(child);
	await new Promise<void>((resolve, reject) => {
		const handleError = (error: Error): void => {
			child.off('spawn', handleSpawn);
			reject(error);
		};
		const handleSpawn = (): void => {
			child.off('error', handleError);
			resolve();
		};
		child.once('error', handleError);
		child.once('spawn', handleSpawn);
	});
	return process;
}

async function runCommand(
	command: string,
	arguments_: readonly string[],
	options: Parameters<typeof spawn>[2],
): Promise<void> {
	const child = spawn(command, [...arguments_], { ...options, stdio: 'inherit' });
	await new Promise<void>((resolve, reject) => {
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0) resolve();
			else
				reject(
					new Error(`${command} exited with code ${String(code)} and signal ${String(signal)}.`),
				);
		});
	});
}

function terminalHostBinaryPath(terminalHostRoot: string): string {
	const binaryName =
		process.platform === 'win32' ? 'orchbee-terminal-host.exe' : 'orchbee-terminal-host';
	return path.join(terminalHostRoot, 'zig-out', 'bin', binaryName);
}

async function fileDigest(filePath: string): Promise<string> {
	return createHash('sha256')
		.update(await readFile(filePath))
		.digest('hex');
}

async function terminalHostInputDigest(terminalHostRoot: string): Promise<string> {
	const sourcePaths = await collectTerminalHostInputPaths(path.join(terminalHostRoot, 'src'));
	const inputPaths = [
		path.join(terminalHostRoot, 'build.zig'),
		path.join(terminalHostRoot, 'build.zig.zon'),
		...sourcePaths,
	].sort();
	const hash = createHash('sha256');
	for (const inputPath of inputPaths) {
		hash.update(path.relative(terminalHostRoot, inputPath).split(path.sep).join('/'));
		hash.update('\0');
		hash.update(await readFile(inputPath));
		hash.update('\0');
	}
	return hash.digest('hex');
}

async function collectTerminalHostInputPaths(directory: string): Promise<string[]> {
	const entries = await readdir(directory, { withFileTypes: true });
	const paths = await Promise.all(
		entries.map(async (entry): Promise<string[]> => {
			const entryPath = path.join(directory, entry.name);
			if (entry.isDirectory()) return collectTerminalHostInputPaths(entryPath);
			return entry.isFile() && ['.c', '.h', '.zig'].includes(path.extname(entry.name))
				? [entryPath]
				: [];
		}),
	);
	return paths.flat();
}

function resolveOptionalDirectory(value: string | undefined, fallback: string): string {
	return path.resolve(value ?? fallback);
}

function resolveScrollbackBytes(value: string | undefined): string {
	const encoded = value ?? '10000000';
	if (!/^(?:0|[1-9][0-9]*)$/u.test(encoded)) {
		throw new Error('ORCHBEE_TERMINAL_SCROLLBACK_BYTES must be a decimal byte count.');
	}
	const parsed = TerminalConfigurationSchema.safeParse({ scrollbackBytes: Number(encoded) });
	if (!parsed.success) {
		throw new Error('ORCHBEE_TERMINAL_SCROLLBACK_BYTES exceeds the supported range.');
	}
	return String(parsed.data.scrollbackBytes);
}

function hasExited(child: ChildProcess): boolean {
	return child.exitCode !== null || child.signalCode !== null;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'unknown failure';
}
