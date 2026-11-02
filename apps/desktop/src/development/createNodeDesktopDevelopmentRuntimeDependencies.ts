import { spawn, type ChildProcess } from 'node:child_process';

import type { DesktopDevelopmentRestartRequest } from './desktop-development-contract';
import type {
	DesktopDevelopmentRuntimeDependencies,
	DesktopDevelopmentRuntimeLogger,
	DesktopDevelopmentSupervisedProcess,
} from './DesktopDevelopmentRuntime';
import { prepareDesktopDevelopmentCohort } from './prepareDesktopDevelopmentCohort';

const PROCESS_TERMINATION_TIMEOUT_MILLISECONDS = 2_000;

/** @internal Development-tool adapter consumed only by the Desktop Rsbuild configuration. */
export function createNodeDesktopDevelopmentRuntimeDependencies(
	logger: DesktopDevelopmentRuntimeLogger,
): DesktopDevelopmentRuntimeDependencies {
	return {
		prepareCohort: ({ environment, sourceRoot }) =>
			prepareDesktopDevelopmentCohort({ environment, logger, sourceRoot }),
		spawnElectron: (input) =>
			spawnSupervisedProcess(input.executable, [input.entryPath], {
				env: input.environment,
				stdio: ['inherit', 'inherit', 'inherit', 'ipc'],
			}),
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

function hasExited(child: ChildProcess): boolean {
	return child.exitCode !== null || child.signalCode !== null;
}
