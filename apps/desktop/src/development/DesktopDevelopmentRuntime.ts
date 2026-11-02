import path from 'node:path';

import type { DesktopDevelopmentRestartRequest } from './desktop-development-contract';

const DESKTOP_EXIT_TIMEOUT_MILLISECONDS = 5_000;
/** Inherited by the Session's shells through `orchbee`, where a non-interactive pager would stick. */
const SHELL_ENVIRONMENT_EXCLUSIONS = new Set(['GIT_PAGER', 'PAGER']);

export interface DesktopDevelopmentRuntimeLogger {
	error(message: string): void;
	info(message: string): void;
	warn(message: string): void;
}

interface DesktopDevelopmentRuntimeOptions {
	readonly desktopEntryPath: string;
	readonly electronExecutable: string;
	readonly environment: NodeJS.ProcessEnv;
	readonly rendererUrl: string;
	readonly sourceRoot: string;
}

export interface DesktopDevelopmentSupervisedProcess {
	readonly connected: boolean;
	send(message: DesktopDevelopmentRestartRequest): Promise<boolean>;
	terminate(): Promise<void>;
	waitForExit(timeoutMilliseconds: number): Promise<boolean>;
}

export interface DesktopDevelopmentRuntimeDependencies {
	/** Builds the checkout's native cohort, reusing an identical staged one, and returns its `orchbee`. */
	prepareCohort(input: {
		readonly environment: NodeJS.ProcessEnv;
		readonly sourceRoot: string;
	}): Promise<string>;
	spawnElectron(input: {
		readonly entryPath: string;
		readonly environment: NodeJS.ProcessEnv;
		readonly executable: string;
	}): Promise<DesktopDevelopmentSupervisedProcess>;
}

/**
 * @internal Development-tool runtime consumed only by the Desktop Rsbuild configuration. It stages
 * the native cohort once, then keeps one Electron process and restarts it after each Main or preload
 * build. The Session it connects to outlives every restart, as it outlives a released Desktop.
 */
export class DesktopDevelopmentRuntime {
	#clientExecutable: string | undefined;
	readonly #dependencies: DesktopDevelopmentRuntimeDependencies;
	#desktopProcess: DesktopDevelopmentSupervisedProcess | undefined;
	readonly #logger: DesktopDevelopmentRuntimeLogger;
	readonly #options: DesktopDevelopmentRuntimeOptions;
	#pendingRestart = false;
	#processing: Promise<void> | undefined;
	#restartRequestGeneration = 0;
	#started = false;
	#stopping = false;

	constructor(
		options: DesktopDevelopmentRuntimeOptions,
		dependencies: DesktopDevelopmentRuntimeDependencies,
		logger: DesktopDevelopmentRuntimeLogger,
	) {
		for (const [name, value] of [
			['Desktop entry', options.desktopEntryPath],
			['Electron executable', options.electronExecutable],
			['Source root', options.sourceRoot],
		] as const) {
			if (!path.isAbsolute(value)) throw new Error(`${name} path must be absolute.`);
		}
		this.#dependencies = dependencies;
		this.#logger = logger;
		this.#options = options;
	}

	async start(): Promise<void> {
		if (this.#started) return;
		if (this.#stopping) throw new Error('Desktop development runtime is stopping.');
		this.#logger.info('Building the native cohort…');
		this.#clientExecutable = await this.#dependencies.prepareCohort({
			environment: this.#options.environment,
			sourceRoot: this.#options.sourceRoot,
		});
		if (this.#stopping) return;
		this.#desktopProcess = await this.#spawnElectron(this.#clientExecutable);
		this.#started = true;
		this.#logger.info('Desktop development runtime is ready.');
		this.#schedule();
	}

	requestDesktopRestart(): void {
		if (this.#stopping) return;
		this.#pendingRestart = true;
		this.#schedule();
	}

	async stop(): Promise<void> {
		if (this.#stopping) return;
		this.#stopping = true;
		this.#pendingRestart = false;
		while (this.#processing !== undefined) await this.#processing;
		const desktop = this.#desktopProcess;
		this.#desktopProcess = undefined;
		await desktop?.terminate().catch(() => undefined);
		this.#started = false;
	}

	#schedule(): void {
		if (!this.#started || this.#stopping || this.#processing !== undefined) return;
		if (!this.#pendingRestart) return;
		const operation = this.#restartPending().catch((error: unknown) => {
			this.#logger.error(`Desktop development restart failed: ${errorMessage(error)}`);
		});
		this.#processing = operation.finally(() => {
			this.#processing = undefined;
			this.#schedule();
		});
	}

	async #restartPending(): Promise<void> {
		const clientExecutable = this.#clientExecutable;
		if (clientExecutable === undefined) return;
		while (this.#pendingRestart && !this.#stopping) {
			this.#pendingRestart = false;
			const desktop = this.#desktopProcess;
			if (desktop !== undefined) {
				this.#logger.info('Restarting Electron after a successful Main or preload build…');
				if (!(await this.#requestDesktopExit(desktop))) await desktop.terminate();
				if (this.#desktopProcess === desktop) this.#desktopProcess = undefined;
			}
			if (!this.#stopping) this.#desktopProcess = await this.#spawnElectron(clientExecutable);
		}
	}

	async #requestDesktopExit(desktop: DesktopDevelopmentSupervisedProcess): Promise<boolean> {
		this.#restartRequestGeneration += 1;
		const sent =
			desktop.connected &&
			(await desktop.send({
				generation: this.#restartRequestGeneration,
				type: 'orchbee-desktop-development-restart-request',
			}));
		return desktop.waitForExit(sent ? DESKTOP_EXIT_TIMEOUT_MILLISECONDS : 0);
	}

	#spawnElectron(clientExecutable: string): Promise<DesktopDevelopmentSupervisedProcess> {
		const environment: NodeJS.ProcessEnv = {};
		for (const [name, value] of Object.entries(this.#options.environment)) {
			if (!SHELL_ENVIRONMENT_EXCLUSIONS.has(name)) environment[name] = value;
		}
		return this.#dependencies.spawnElectron({
			entryPath: this.#options.desktopEntryPath,
			environment: {
				...environment,
				ORCHBEE_DESKTOP_CLIENT_EXECUTABLE: clientExecutable,
				ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: this.#options.sourceRoot,
				ORCHBEE_DESKTOP_RENDERER_URL: this.#options.rendererUrl,
			},
			executable: this.#options.electronExecutable,
		});
	}
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'unknown failure';
}
