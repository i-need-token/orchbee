import path from 'node:path';

import {
	DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY_ENVIRONMENT_KEY,
	DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_TRUST_DESCRIPTOR_ENVIRONMENT_KEY,
	type DesktopDevelopmentRestartRequest,
} from './desktop-development-contract';
import type {
	DesktopDevelopmentRestartPlanLoadResult,
	DesktopDevelopmentRestartPlanStore,
} from './DesktopDevelopmentRestartPlanStore';

const DEFAULT_RESTART_TIMEOUT_MILLISECONDS = 15_000;
const DESKTOP_RESTART_TIMEOUT_MILLISECONDS = 5_000;

export interface DesktopDevelopmentRuntimeLogger {
	error(message: string): void;
	info(message: string): void;
	warn(message: string): void;
}

interface DesktopDevelopmentRuntimeOptions {
	readonly desktopEntryPath: string;
	readonly electronExecutable: string;
	readonly environment: NodeJS.ProcessEnv;
	readonly gatewayEntryPath: string;
	readonly rendererUrl: string;
	readonly restartTimeoutMilliseconds?: number;
	readonly serverEntryPath: string;
	readonly sourceRoot: string;
	readonly sshEnvironmentRelease?:
		| {
				readonly releaseDirectoryPath: string;
				readonly trustDescriptorFilePath: string;
		  }
		| undefined;
	readonly terminalHostRoot: string;
	readonly webRendererRoot: string;
}

export interface DesktopDevelopmentWorkspace {
	readonly endpointId: string;
	readonly recoveryDirectory: string;
	readonly restartPlanPath: string;
	readonly runtimeDirectory: string;
	readonly terminalScrollbackBytes: string;
}

export interface DesktopDevelopmentHostBuild {
	readonly binaryPath: string;
	readonly digest: string;
	readonly inputDigest: string;
}

/** Development-tool contract consumed only by the Desktop build supervisor. */
export interface DesktopDevelopmentHostCandidate extends DesktopDevelopmentHostBuild {
	readonly generation: number;
}

export interface DesktopDevelopmentSupervisedProcess {
	readonly connected: boolean;
	send(message: DesktopDevelopmentRestartRequest): Promise<boolean>;
	terminate(): Promise<void>;
	waitForExit(timeoutMilliseconds: number): Promise<boolean>;
}

export interface DesktopDevelopmentHostBuildWatch {
	close(): Promise<void>;
}

type RestartPlanControl = Pick<
	DesktopDevelopmentRestartPlanStore,
	'load' | 'remove' | 'writePending'
>;

export interface DesktopDevelopmentRuntimeDependencies {
	buildInitialHost(terminalHostRoot: string): Promise<DesktopDevelopmentHostBuild>;
	captureHostCandidate(
		build: DesktopDevelopmentHostBuild,
		workspace: DesktopDevelopmentWorkspace,
		generation: number,
	): Promise<DesktopDevelopmentHostCandidate>;
	cleanupWorkspace(workspace: DesktopDevelopmentWorkspace): Promise<void>;
	createRestartPlan(filePath: string): RestartPlanControl;
	createWorkspace(environment: NodeJS.ProcessEnv): Promise<DesktopDevelopmentWorkspace>;
	releaseHostCandidate(candidate: DesktopDevelopmentHostCandidate): Promise<void>;
	spawnElectron(input: {
		readonly entryPath: string;
		readonly environment: NodeJS.ProcessEnv;
		readonly executable: string;
	}): Promise<DesktopDevelopmentSupervisedProcess>;
	spawnHost(input: {
		readonly candidate: DesktopDevelopmentHostCandidate;
		readonly workspace: DesktopDevelopmentWorkspace;
	}): Promise<DesktopDevelopmentSupervisedProcess>;
	watchHostBuilds(
		terminalHostRoot: string,
		initialBuild: DesktopDevelopmentHostBuild,
		onBuild: (build: DesktopDevelopmentHostBuild) => Promise<void>,
	): Promise<DesktopDevelopmentHostBuildWatch>;
}

/** @internal Development-tool runtime consumed only by the Desktop Rsbuild configuration. */
export class DesktopDevelopmentRuntime {
	readonly #dependencies: DesktopDevelopmentRuntimeDependencies;
	#desktopProcess: DesktopDevelopmentSupervisedProcess | undefined;
	#hostBuildWatch: DesktopDevelopmentHostBuildWatch | undefined;
	#hostGeneration = 0;
	#hostProcess: DesktopDevelopmentSupervisedProcess | undefined;
	#activeHostCandidate: DesktopDevelopmentHostCandidate | undefined;
	readonly #logger: DesktopDevelopmentRuntimeLogger;
	readonly #options: DesktopDevelopmentRuntimeOptions;
	#pendingDesktopRestart = false;
	#pendingHostCandidate: DesktopDevelopmentHostCandidate | undefined;
	#processing: Promise<void> | undefined;
	#restartRequestGeneration = 0;
	readonly #restartTimeoutMilliseconds: number;
	#restartPlan: RestartPlanControl | undefined;
	#started = false;
	#stopping = false;
	#workspace: DesktopDevelopmentWorkspace | undefined;

	constructor(
		options: DesktopDevelopmentRuntimeOptions,
		dependencies: DesktopDevelopmentRuntimeDependencies,
		logger: DesktopDevelopmentRuntimeLogger,
	) {
		for (const [name, value] of [
			['Desktop entry', options.desktopEntryPath],
			['Electron executable', options.electronExecutable],
			['Gateway entry', options.gatewayEntryPath],
			['Server entry', options.serverEntryPath],
			['Source root', options.sourceRoot],
			['Terminal Host root', options.terminalHostRoot],
			['Web renderer root', options.webRendererRoot],
		] as const) {
			if (!path.isAbsolute(value)) throw new Error(`${name} path must be absolute.`);
		}
		if (options.sshEnvironmentRelease !== undefined) {
			for (const [name, value] of [
				['Development SSH Environment release', options.sshEnvironmentRelease.releaseDirectoryPath],
				[
					'Development SSH Environment trust descriptor',
					options.sshEnvironmentRelease.trustDescriptorFilePath,
				],
			] as const) {
				if (!path.isAbsolute(value)) throw new Error(`${name} path must be absolute.`);
			}
		}
		this.#dependencies = dependencies;
		this.#logger = logger;
		this.#options = options;
		this.#restartTimeoutMilliseconds =
			options.restartTimeoutMilliseconds ?? DEFAULT_RESTART_TIMEOUT_MILLISECONDS;
		if (this.#restartTimeoutMilliseconds < 1) {
			throw new Error('Desktop development restart timeout must be positive.');
		}
	}

	async start(): Promise<void> {
		if (this.#started) return;
		if (this.#stopping) throw new Error('Desktop development runtime is stopping.');
		const workspace = await this.#dependencies.createWorkspace(this.#options.environment);
		this.#workspace = workspace;
		this.#restartPlan = this.#dependencies.createRestartPlan(workspace.restartPlanPath);
		try {
			this.#logger.info('Building the initial supervised Terminal Host…');
			const build = await this.#dependencies.buildInitialHost(this.#options.terminalHostRoot);
			const initialCandidate = await this.#captureCandidate(build);
			this.#activeHostCandidate = initialCandidate;
			this.#hostBuildWatch = await this.#dependencies.watchHostBuilds(
				this.#options.terminalHostRoot,
				build,
				this.#acceptHostBuild,
			);
			const startupCandidate = this.#pendingHostCandidate ?? initialCandidate;
			this.#pendingHostCandidate = undefined;
			if (startupCandidate !== initialCandidate) {
				await this.#releaseHostCandidate(initialCandidate);
			}
			this.#activeHostCandidate = startupCandidate;
			this.#hostProcess = await this.#dependencies.spawnHost({
				candidate: startupCandidate,
				workspace,
			});
			this.#desktopProcess = await this.#spawnElectron(startupCandidate);
			this.#started = true;
			this.#logger.info('Desktop development runtime is ready.');
			this.#schedule();
		} catch (error: unknown) {
			await this.#closeHostBuildWatch();
			await this.#disposeOwnedResources();
			throw error;
		}
	}

	requestDesktopRestart(): void {
		if (this.#stopping) return;
		this.#pendingDesktopRestart = true;
		this.#schedule();
	}

	async waitForIdle(): Promise<void> {
		while (this.#processing !== undefined) await this.#processing;
	}

	async stop(): Promise<void> {
		if (this.#stopping) return;
		this.#stopping = true;
		this.#pendingDesktopRestart = false;
		await this.#closeHostBuildWatch();
		await this.waitForIdle();
		await this.#disposeOwnedResources();
		this.#started = false;
	}

	readonly #acceptHostBuild = async (build: DesktopDevelopmentHostBuild): Promise<void> => {
		if (this.#stopping) return;
		const candidate = await this.#captureCandidate(build);
		if (this.#stopping) {
			await this.#releaseHostCandidate(candidate);
			return;
		}
		const supersededCandidate = this.#pendingHostCandidate;
		this.#pendingHostCandidate = candidate;
		await this.#releaseHostCandidate(supersededCandidate);
		this.#logger.info(`Terminal Host generation ${candidate.generation} is ready to apply.`);
		this.#schedule();
	};

	async #captureCandidate(
		build: DesktopDevelopmentHostBuild,
	): Promise<DesktopDevelopmentHostCandidate> {
		const workspace = this.#requireWorkspace();
		this.#hostGeneration += 1;
		return this.#dependencies.captureHostCandidate(build, workspace, this.#hostGeneration);
	}

	#schedule(): void {
		if (!this.#started || this.#stopping || this.#processing !== undefined) return;
		const operation = this.#processUpdates().catch((error: unknown) => {
			this.#logger.error(`Desktop development update failed: ${errorMessage(error)}`);
		});
		this.#processing = operation.finally(() => {
			this.#processing = undefined;
			if (
				!this.#stopping &&
				(this.#pendingHostCandidate !== undefined || this.#pendingDesktopRestart)
			) {
				this.#schedule();
			}
		});
	}

	async #processUpdates(): Promise<void> {
		while (!this.#stopping) {
			const hostCandidate = this.#pendingHostCandidate;
			if (hostCandidate !== undefined) {
				this.#pendingHostCandidate = undefined;
				const desktopRestartWasPending = this.#pendingDesktopRestart;
				this.#pendingDesktopRestart = false;
				let applied = false;
				try {
					applied = await this.#applyHostCandidate(hostCandidate);
				} catch (error: unknown) {
					this.#logger.error(
						`Terminal Host generation ${hostCandidate.generation} update failed: ${errorMessage(error)}`,
					);
				} finally {
					if (this.#activeHostCandidate !== hostCandidate) {
						await this.#releaseHostCandidate(hostCandidate);
					}
				}
				if (!applied && desktopRestartWasPending) this.#pendingDesktopRestart = true;
				continue;
			}
			if (!this.#pendingDesktopRestart) return;
			this.#pendingDesktopRestart = false;
			await this.#restartDesktop();
		}
	}

	async #restartDesktop(): Promise<void> {
		const activeCandidate = this.#activeHostCandidate;
		if (activeCandidate === undefined) return;
		const desktop = this.#desktopProcess;
		if (desktop !== undefined) {
			this.#logger.info('Restarting Electron after a successful Main or preload build…');
			const exited = await this.#requestDesktopExit(desktop, 'desktop');
			if (!exited) await desktop.terminate();
			if (this.#desktopProcess === desktop) this.#desktopProcess = undefined;
		}
		if (!this.#stopping) this.#desktopProcess = await this.#spawnElectron(activeCandidate);
	}

	async #applyHostCandidate(candidate: DesktopDevelopmentHostCandidate): Promise<boolean> {
		const restartPlan = this.#restartPlan;
		const desktop = this.#desktopProcess;
		const oldHost = this.#hostProcess;
		const oldCandidate = this.#activeHostCandidate;
		if (
			restartPlan === undefined ||
			desktop === undefined ||
			oldHost === undefined ||
			oldCandidate === undefined
		) {
			this.#logger.warn(
				'Terminal Host update was skipped because the current runtime is incomplete.',
			);
			return false;
		}
		await restartPlan.writePending(candidate.generation);
		this.#logger.info(
			`Preparing terminal sessions for Terminal Host generation ${candidate.generation}…`,
		);
		let desktopExited = await this.#requestDesktopExit(desktop, 'terminal-host');
		const plan = await restartPlan.load();
		const ready = isReadyPlan(plan, candidate.generation);
		if (ready && !desktopExited) {
			await desktop.terminate();
			desktopExited = true;
		}
		if (!ready) {
			await restartPlan.remove().catch(() => undefined);
			this.#logger.warn(
				`Terminal Host generation ${candidate.generation} was not applied because restart preparation did not become durable.`,
			);
			if (desktopExited && !this.#stopping) {
				this.#desktopProcess = await this.#spawnElectron(oldCandidate);
			}
			return desktopExited;
		}
		if (!desktopExited) {
			await restartPlan.remove().catch(() => undefined);
			this.#logger.warn('Terminal Host restart preparation completed, but Electron did not stop.');
			return false;
		}
		if (this.#desktopProcess === desktop) this.#desktopProcess = undefined;
		if (!(await oldHost.waitForExit(2_000))) await oldHost.terminate();
		if (this.#hostProcess === oldHost) this.#hostProcess = undefined;
		if (this.#stopping) return false;
		let selectedCandidate = candidate;
		try {
			this.#hostProcess = await this.#dependencies.spawnHost({
				candidate,
				workspace: this.#requireWorkspace(),
			});
		} catch (error: unknown) {
			this.#logger.error(
				`Terminal Host generation ${candidate.generation} failed to start: ${errorMessage(error)}`,
			);
			selectedCandidate = oldCandidate;
			this.#hostProcess = await this.#dependencies.spawnHost({
				candidate: oldCandidate,
				workspace: this.#requireWorkspace(),
			});
		}
		this.#activeHostCandidate = selectedCandidate;
		await restartPlan.remove().catch(() => undefined);
		this.#desktopProcess = await this.#spawnElectron(selectedCandidate);
		if (selectedCandidate === candidate) await this.#releaseHostCandidate(oldCandidate);
		this.#logger.info(
			selectedCandidate === candidate
				? `Terminal Host generation ${candidate.generation} is running.`
				: `Terminal Host generation ${candidate.generation} rolled back to the last runnable binary.`,
		);
		return true;
	}

	async #requestDesktopExit(
		desktop: DesktopDevelopmentSupervisedProcess,
		reason: DesktopDevelopmentRestartRequest['reason'],
	): Promise<boolean> {
		this.#restartRequestGeneration += 1;
		const request: DesktopDevelopmentRestartRequest = {
			generation: this.#restartRequestGeneration,
			reason,
			type: 'orchbee-desktop-development-restart-request',
		};
		const sent = desktop.connected && (await desktop.send(request));
		const timeout =
			reason === 'desktop'
				? Math.min(this.#restartTimeoutMilliseconds, DESKTOP_RESTART_TIMEOUT_MILLISECONDS)
				: this.#restartTimeoutMilliseconds;
		return desktop.waitForExit(sent ? timeout : 0);
	}

	#spawnElectron(
		candidate: DesktopDevelopmentHostCandidate,
	): Promise<DesktopDevelopmentSupervisedProcess> {
		const workspace = this.#requireWorkspace();
		return this.#dependencies.spawnElectron({
			entryPath: this.#options.desktopEntryPath,
			environment: {
				...this.#options.environment,
				ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN: workspace.restartPlanPath,
				ORCHBEE_DESKTOP_ENDPOINT_ID: workspace.endpointId,
				ORCHBEE_DESKTOP_GATEWAY: this.#options.environment['ORCHBEE_DESKTOP_GATEWAY'] ?? '1',
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: workspace.recoveryDirectory,
				ORCHBEE_DESKTOP_RENDERER_URL: this.#options.rendererUrl,
				ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: workspace.runtimeDirectory,
				ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: this.#options.sourceRoot,
				...(this.#options.sshEnvironmentRelease === undefined
					? {}
					: {
							[DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY_ENVIRONMENT_KEY]:
								this.#options.sshEnvironmentRelease.releaseDirectoryPath,
							[DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_TRUST_DESCRIPTOR_ENVIRONMENT_KEY]:
								this.#options.sshEnvironmentRelease.trustDescriptorFilePath,
						}),
				ORCHBEE_GATEWAY_ENTRY_PATH: this.#options.gatewayEntryPath,
				ORCHBEE_SERVER_ENTRY_PATH: this.#options.serverEntryPath,
				ORCHBEE_TERMINAL_HOST_BINARY: candidate.binaryPath,
				ORCHBEE_TERMINAL_HOST_SUPERVISED: '1',
				ORCHBEE_TERMINAL_SCROLLBACK_BYTES: workspace.terminalScrollbackBytes,
				ORCHBEE_WEB_RENDERER_ROOT: this.#options.webRendererRoot,
			},
			executable: this.#options.electronExecutable,
		});
	}

	async #closeHostBuildWatch(): Promise<void> {
		const hostBuildWatch = this.#hostBuildWatch;
		this.#hostBuildWatch = undefined;
		await hostBuildWatch?.close().catch(() => undefined);
	}

	async #releaseHostCandidate(
		candidate: DesktopDevelopmentHostCandidate | undefined,
	): Promise<void> {
		if (candidate === undefined) return;
		await this.#dependencies.releaseHostCandidate(candidate).catch((error: unknown) => {
			this.#logger.warn(
				`Could not release Terminal Host generation ${candidate.generation}: ${errorMessage(error)}`,
			);
		});
	}

	async #disposeOwnedResources(): Promise<void> {
		this.#pendingDesktopRestart = false;
		const pendingCandidate = this.#pendingHostCandidate;
		this.#pendingHostCandidate = undefined;
		const desktop = this.#desktopProcess;
		this.#desktopProcess = undefined;
		await desktop?.terminate().catch(() => undefined);
		const host = this.#hostProcess;
		this.#hostProcess = undefined;
		await host?.terminate().catch(() => undefined);
		const activeCandidate = this.#activeHostCandidate;
		this.#activeHostCandidate = undefined;
		await this.#releaseHostCandidate(pendingCandidate);
		if (activeCandidate !== pendingCandidate) {
			await this.#releaseHostCandidate(activeCandidate);
		}
		await this.#restartPlan?.remove().catch(() => undefined);
		this.#restartPlan = undefined;
		const workspace = this.#workspace;
		this.#workspace = undefined;
		if (workspace !== undefined) {
			await this.#dependencies.cleanupWorkspace(workspace).catch(() => undefined);
		}
	}

	#requireWorkspace(): DesktopDevelopmentWorkspace {
		if (this.#workspace === undefined) {
			throw new Error('Desktop development workspace is unavailable.');
		}
		return this.#workspace;
	}
}

function isReadyPlan(result: DesktopDevelopmentRestartPlanLoadResult, generation: number): boolean {
	return (
		result.kind === 'available' &&
		result.plan.state === 'ready' &&
		result.plan.generation === generation
	);
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'unknown failure';
}
