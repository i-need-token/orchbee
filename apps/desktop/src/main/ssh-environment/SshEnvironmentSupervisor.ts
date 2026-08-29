import { randomInt } from 'node:crypto';

import {
	SshEnvironmentRuntimeInstanceIdSchema,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import type { OpenSshTargetResolver } from './OpenSshTargetResolver';
import type { OpenSshTransport, OpenSshTransportAdapter } from './OpenSshTransportAdapter';
import {
	SshAskPassBridge,
	type SshAskPassBridgeOptions,
	type SshAskPassInteractionRequest,
	type SshAskPassInteractionResult,
} from './SshAskPassBridge';
import {
	SshEnvironmentRuntimeSession,
	type SshEnvironmentRuntimeIdentity,
	type SshEnvironmentRuntimeSessionFailureReason,
} from './SshEnvironmentRuntimeSession';
import type { SshProjectRootInspectionOutcome } from './SshProjectRootChannelClient';
import type { SshRuntimeReleaseSource } from './ssh-runtime-release-source';
import { createSshRuntimeRemoteBootstrap } from './createSshRuntimeRemoteBootstrap';
import {
	createSshEnvironmentConnectionState,
	observeSshEnvironmentConnection,
	transitionSshEnvironmentConnection,
	type SshEnvironmentConnectionCommand,
	type SshEnvironmentConnectionConfiguration,
	type SshEnvironmentConnectionEvent,
	type SshEnvironmentConnectionObservation,
	type SshEnvironmentConnectionState,
} from './ssh-environment-connection-state';

export interface SshEnvironmentSupervisorConfiguration extends SshEnvironmentConnectionConfiguration {
	readonly askPassHelperEntryPath: string;
	readonly askPassRuntimeExecutablePath: string;
	readonly runtimeInstanceId: string;
}

interface AskPassBridgePort {
	dispose(): Promise<void>;
	start(): Promise<{ readonly environment: Readonly<NodeJS.ProcessEnv> }>;
}

interface RuntimeSessionPort {
	inspectProjectRoot(path: string, signal?: AbortSignal): Promise<SshProjectRootInspectionOutcome>;
	start(): Promise<void>;
	stop(): void;
}

export interface SshEnvironmentSupervisorDependencies {
	readonly releaseSource: SshRuntimeReleaseSource;
	readonly askPassBridgeFactory?: (options: SshAskPassBridgeOptions) => AskPassBridgePort;
	readonly interaction: (
		request: SshAskPassInteractionRequest,
	) => Promise<SshAskPassInteractionResult>;
	readonly randomDelay?: (minimum: number, maximum: number) => number;
	readonly runtimeSessionFactory?: (
		options: ConstructorParameters<typeof SshEnvironmentRuntimeSession>[0],
	) => RuntimeSessionPort;
	readonly targetResolver: Pick<OpenSshTargetResolver, 'resolve'>;
	readonly transportAdapter: Pick<OpenSshTransportAdapter, 'open'>;
}

interface GenerationResources {
	askPass?: AskPassBridgePort;
	authenticationPrompted: boolean;
	hostVerificationPrompted: boolean;
	ready: boolean;
	retired: boolean;
	retryTimer?: NodeJS.Timeout;
	session?: RuntimeSessionPort;
	terminalNotified: boolean;
	transport?: OpenSshTransport;
}

export class SshEnvironmentSupervisor {
	readonly #configuration: SshEnvironmentSupervisorConfiguration & {
		readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	};
	readonly #dependencies: Required<
		Pick<
			SshEnvironmentSupervisorDependencies,
			'askPassBridgeFactory' | 'randomDelay' | 'runtimeSessionFactory'
		>
	> &
		Omit<
			SshEnvironmentSupervisorDependencies,
			'askPassBridgeFactory' | 'randomDelay' | 'runtimeSessionFactory'
		>;
	readonly #listeners = new Set<(observation: SshEnvironmentConnectionObservation) => void>();
	readonly #pendingRetirements = new Set<Promise<void>>();
	readonly #resources = new Map<number, GenerationResources>();
	#state: SshEnvironmentConnectionState;

	constructor(
		configuration: SshEnvironmentSupervisorConfiguration,
		dependencies: SshEnvironmentSupervisorDependencies,
	) {
		this.#configuration = {
			...configuration,
			runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema.parse(
				configuration.runtimeInstanceId,
			),
		};
		this.#dependencies = {
			...dependencies,
			askPassBridgeFactory:
				dependencies.askPassBridgeFactory ?? ((options) => new SshAskPassBridge(options)),
			randomDelay:
				dependencies.randomDelay ?? ((minimum, maximum) => randomInt(minimum, maximum + 1)),
			runtimeSessionFactory:
				dependencies.runtimeSessionFactory ??
				((options) => new SshEnvironmentRuntimeSession(options)),
		};
		this.#state = createSshEnvironmentConnectionState(configuration);
	}

	observation(): SshEnvironmentConnectionObservation {
		return observeSshEnvironmentConnection(this.#state);
	}

	subscribe(listener: (observation: SshEnvironmentConnectionObservation) => void): () => void {
		this.#listeners.add(listener);
		listener(this.observation());
		return () => this.#listeners.delete(listener);
	}

	start(): void {
		this.#dispatch({ type: 'start-requested' });
	}

	async stop(): Promise<void> {
		this.#dispatch({ type: 'stop-requested' });
		await Promise.all(this.#pendingRetirements);
	}

	inspectProjectRoot(path: string, signal?: AbortSignal): Promise<SshProjectRootInspectionOutcome> {
		const resources = this.#resources.get(this.#state.generation);
		if (this.#state.phase !== 'connected' || resources?.ready !== true) {
			return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		}
		return (
			resources.session?.inspectProjectRoot(path, signal) ??
			Promise.resolve({ kind: 'failed', reason: 'unavailable' })
		);
	}

	#dispatch(event: SshEnvironmentConnectionEvent): void {
		const commands = this.#transition(event);
		for (const command of commands) this.#execute(command);
	}

	#transition(event: SshEnvironmentConnectionEvent): readonly SshEnvironmentConnectionCommand[] {
		const transition = transitionSshEnvironmentConnection(this.#state, event);
		if (transition.outcome === 'unchanged') return [];
		this.#state = transition.state;
		const observation = this.observation();
		for (const listener of this.#listeners) listener(observation);
		return transition.commands;
	}

	#execute(command: SshEnvironmentConnectionCommand): void {
		switch (command.type) {
			case 'resolve-target':
				void this.#resolveTarget(command);
				return;
			case 'open-transport':
				void this.#openTransport(command);
				return;
			case 'present-interaction':
				// AskPass owns the transient prompt and executes this command synchronously
				// through #requestInteraction so no secret enters the command model.
				return;
			case 'retire-generation':
				this.#trackRetirement(this.#retireGeneration(command.generation));
				return;
			case 'schedule-retry': {
				const resources = this.#resources.get(command.generation) ?? createResources();
				this.#resources.set(command.generation, resources);
				const delay = this.#dependencies.randomDelay(
					command.minimumDelayMilliseconds,
					command.maximumDelayMilliseconds,
				);
				resources.retryTimer = setTimeout(
					() => this.#dispatch({ generation: command.generation, type: 'retry-elapsed' }),
					delay,
				);
			}
		}
	}

	async #resolveTarget(
		command: Extract<SshEnvironmentConnectionCommand, { readonly type: 'resolve-target' }>,
	): Promise<void> {
		const result = await this.#dependencies.targetResolver.resolve(command.target);
		if (!this.#isCurrentGeneration(command.generation)) return;
		if (result.kind === 'resolved') {
			this.#dispatch({
				generation: command.generation,
				resolvedTarget: result.target,
				type: 'target-resolved',
			});
			return;
		}
		this.#dispatch({
			generation: command.generation,
			reason: mapTargetResolutionFailure(result.code),
			type: 'target-resolution-failed',
		});
	}

	async #openTransport(
		command: Extract<SshEnvironmentConnectionCommand, { readonly type: 'open-transport' }>,
	): Promise<void> {
		const resources = createResources();
		this.#resources.set(command.generation, resources);
		try {
			const release = await this.#dependencies.releaseSource.loadCatalog();
			if (!this.#isLive(command.generation, resources)) return;
			if (release.kind === 'unavailable') {
				this.#transportFailed(command.generation, 'runtime-release-unavailable');
				return;
			}

			const bootstrap = createSshRuntimeRemoteBootstrap({
				catalog: release.catalog,
				runtimeInstanceId: this.#configuration.runtimeInstanceId,
			});
			const askPass = this.#dependencies.askPassBridgeFactory({
				helperEntryPath: this.#configuration.askPassHelperEntryPath,
				interaction: (request) => this.#requestInteraction(command.generation, request),
				runtimeExecutablePath: this.#configuration.askPassRuntimeExecutablePath,
			});
			resources.askPass = askPass;
			const askPassLaunch = await askPass.start();
			if (!this.#isLive(command.generation, resources)) return;

			const transport = this.#dependencies.transportAdapter.open({
				askPass: askPassLaunch,
				remoteCommand: bootstrap.command,
				resolvedTarget: command.resolvedTarget,
				target: this.#state.target,
			});
			resources.transport = transport;
			transport.onExit((exit) => this.#transportExited(command.generation, resources, exit));
			const session = this.#dependencies.runtimeSessionFactory({
				catalog: release.catalog,
				loadArtifact: (platform) => this.#dependencies.releaseSource.loadArtifact(platform),
				onFailure: (reason) => this.#sessionFailed(command.generation, resources, reason),
				onLost: () => this.#sessionLost(command.generation, resources),
				onReady: (identity) => this.#sessionReady(command.generation, resources, identity),
				readable: transport.readable,
				runtimeInstanceId: bootstrap.runtimeInstanceId,
				writable: transport.writable,
			});
			resources.session = session;
			await session.start();
		} catch {
			this.#transportFailed(command.generation, 'retryable');
		}
	}

	async #requestInteraction(
		generation: number,
		request: SshAskPassInteractionRequest,
	): Promise<SshAskPassInteractionResult> {
		if (!this.#isCurrentGeneration(generation)) return { kind: 'cancelled' };
		const resources = this.#resources.get(generation);
		if (resources !== undefined) {
			if (request.kind === 'confirm') resources.hostVerificationPrompted = true;
			else resources.authenticationPrompted = true;
		}
		const commands = this.#transition({
			generation,
			handle: request.handle,
			type: 'interaction-requested',
		});
		if (!commands.some((command) => command.type === 'present-interaction')) {
			return { kind: 'cancelled' };
		}
		let result: SshAskPassInteractionResult;
		try {
			result = await this.#dependencies.interaction(request);
		} catch {
			result = { kind: 'cancelled' };
		}
		const completion = this.#transition({
			generation,
			handle: request.handle,
			outcome: result.kind === 'submitted' ? 'submitted' : 'cancelled',
			type: 'interaction-completed',
		});
		for (const command of completion) this.#execute(command);
		return this.#isCurrentGeneration(generation) && result.kind === 'submitted'
			? result
			: { kind: 'cancelled' };
	}

	#sessionReady(
		generation: number,
		resources: GenerationResources,
		identity: SshEnvironmentRuntimeIdentity,
	): void {
		if (!this.#isLive(generation, resources)) return;
		resources.ready = true;
		this.#dispatch({ generation, ...identity, type: 'remote-runtime-ready' });
	}

	#sessionFailed(
		generation: number,
		resources: GenerationResources,
		reason: SshEnvironmentRuntimeSessionFailureReason,
	): void {
		if (!this.#isLive(generation, resources) || resources.terminalNotified) return;
		resources.terminalNotified = true;
		this.#transportFailed(generation, reason);
	}

	#sessionLost(generation: number, resources: GenerationResources): void {
		if (!this.#isLive(generation, resources) || resources.terminalNotified) return;
		resources.terminalNotified = true;
		this.#dispatch({ generation, type: 'transport-lost' });
	}

	#transportExited(
		generation: number,
		resources: GenerationResources,
		exit: { readonly code: null | number; readonly signal: null | NodeJS.Signals },
	): void {
		if (!this.#isLive(generation, resources) || resources.terminalNotified) return;
		resources.terminalNotified = true;
		if (resources.ready) this.#dispatch({ generation, type: 'transport-lost' });
		else {
			this.#transportFailed(
				generation,
				classifyBootstrapExit(exit, resources, resources.transport?.diagnostics() ?? ''),
			);
		}
	}

	#transportFailed(generation: number, reason: SupervisorTransportFailureReason): void {
		if (!this.#isCurrentGeneration(generation)) return;
		this.#dispatch({ generation, reason, type: 'transport-failed' });
	}

	async #retireGeneration(generation: number): Promise<void> {
		const resources = this.#resources.get(generation);
		if (resources === undefined || resources.retired) return;
		resources.retired = true;
		if (resources.retryTimer !== undefined) clearTimeout(resources.retryTimer);
		resources.session?.stop();
		resources.transport?.retire();
		await resources.askPass?.dispose();
		if (this.#resources.get(generation) === resources) this.#resources.delete(generation);
	}

	#trackRetirement(retirement: Promise<void>): void {
		this.#pendingRetirements.add(retirement);
		void retirement.finally(() => this.#pendingRetirements.delete(retirement));
	}

	#isCurrentGeneration(generation: number): boolean {
		return generation === this.#state.generation;
	}

	#isLive(generation: number, resources: GenerationResources): boolean {
		return this.#isCurrentGeneration(generation) && !resources.retired;
	}
}

type SupervisorTransportFailureReason =
	| SshEnvironmentRuntimeSessionFailureReason
	| 'authentication-required'
	| 'host-verification-failed';

function classifyBootstrapExit(
	exit: {
		readonly code: null | number;
		readonly signal: null | NodeJS.Signals;
	},
	resources: GenerationResources,
	diagnostics: string,
): SupervisorTransportFailureReason {
	if (exit.signal !== null) return 'retryable';
	switch (exit.code) {
		case 71:
			return 'compatibility-unsupported';
		case 74:
		case 78:
			return 'policy-rejected';
		case 255: {
			if (resources.authenticationPrompted) return 'authentication-required';
			if (resources.hostVerificationPrompted) return 'host-verification-failed';
			const normalized = diagnostics.toLowerCase();
			if (
				normalized.includes('permission denied') ||
				normalized.includes('too many authentication failures')
			) {
				return 'authentication-required';
			}
			if (
				normalized.includes('host key verification failed') ||
				normalized.includes('remote host identification has changed')
			) {
				return 'host-verification-failed';
			}
			return 'retryable';
		}
		default:
			return 'retryable';
	}
}

function createResources(): GenerationResources {
	return {
		authenticationPrompted: false,
		hostVerificationPrompted: false,
		ready: false,
		retired: false,
		terminalNotified: false,
	};
}

function mapTargetResolutionFailure(
	code: Exclude<
		Awaited<ReturnType<OpenSshTargetResolver['resolve']>>,
		{ readonly kind: 'resolved' }
	>['code'],
): 'compatibility-unsupported' | 'configuration-invalid' | 'policy-rejected' {
	switch (code) {
		case 'openssh-unavailable':
		case 'openssh-path-not-absolute':
			return 'compatibility-unsupported';
		case 'host-key-checking-disabled':
			return 'policy-rejected';
		default:
			return 'configuration-invalid';
	}
}
