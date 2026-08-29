import {
	EnvironmentIdSchema,
	type DesktopEnvironmentRegistry,
	type EnvironmentId,
} from '../environment';
import type {
	SshEnvironmentConnectionObservation,
	SshEnvironmentSupervisorInteractionRequest,
	SshAskPassInteractionResult,
	SshProjectRootDirectoryListingOutcome,
	SshProjectRootInspectionOutcome,
	SshTerminalHostConnectionOutcome,
} from '../ssh-environment';

export interface DesktopSshEnvironmentSupervisorPort {
	inspectProjectRoot(path: string, signal?: AbortSignal): Promise<SshProjectRootInspectionOutcome>;
	listProjectDirectory(
		path: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome>;
	openTerminalHost(): Promise<SshTerminalHostConnectionOutcome>;
	observation(): SshEnvironmentConnectionObservation;
	start(): void;
	stop(): Promise<void>;
	subscribe(listener: (observation: SshEnvironmentConnectionObservation) => void): () => void;
}

interface DesktopSshEnvironmentConnectionConfiguration {
	readonly connectionId: string;
	readonly runtimeInstanceId: string;
	readonly target: {
		readonly configurationFilePath: null | string;
		readonly destination: string;
	};
}

export interface DesktopSshEnvironmentConnectionOwnerOptions {
	readonly createSupervisor: (
		configuration: DesktopSshEnvironmentConnectionConfiguration,
		interaction: DesktopSshEnvironmentInteraction,
	) => DesktopSshEnvironmentSupervisorPort;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'get'>;
	readonly supported: boolean;
}

export type DesktopSshEnvironmentInteraction = (
	request: SshEnvironmentSupervisorInteractionRequest,
) => Promise<SshAskPassInteractionResult>;

interface DesktopSshEnvironmentInteractionLease {
	release(): void;
}

export type DesktopSshEnvironmentInteractionLeaseResult =
	| { readonly kind: 'claimed'; readonly lease: DesktopSshEnvironmentInteractionLease }
	| { readonly kind: 'busy' }
	| { readonly kind: 'unavailable'; readonly reason: 'connection-owner-disposed' };

export type DesktopSshEnvironmentConnectResult =
	| {
			readonly activation: 'existing' | 'started';
			readonly kind: 'ready';
			readonly observation: SshEnvironmentConnectionObservation;
	  }
	| { readonly kind: 'not-found' }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'connection-owner-disposed'
				| 'local-platform-unsupported'
				| 'registry-unavailable';
	  };

export type DesktopSshEnvironmentDisconnectResult =
	| { readonly kind: 'disconnected' | 'inactive' }
	| { readonly kind: 'unavailable'; readonly reason: 'connection-owner-disposed' };

export type DesktopSshEnvironmentConnectionReadResult =
	| { readonly kind: 'inactive' }
	| {
			readonly kind: 'ready';
			readonly observation: SshEnvironmentConnectionObservation;
	  };

export interface DesktopSshEnvironmentConnectionOwnerPort {
	claimInteractionLease(
		environmentId: EnvironmentId,
		interaction: DesktopSshEnvironmentInteraction,
	): DesktopSshEnvironmentInteractionLeaseResult;
	connect(environmentId: EnvironmentId): Promise<DesktopSshEnvironmentConnectResult>;
	disconnect(environmentId: EnvironmentId): Promise<DesktopSshEnvironmentDisconnectResult>;
	dispose(): Promise<void>;
	inspectProjectRoot(
		environmentId: EnvironmentId,
		path: string,
		signal?: AbortSignal,
	): Promise<SshProjectRootInspectionOutcome>;
	listProjectDirectory(
		environmentId: EnvironmentId,
		path: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome>;
	observation(environmentId: EnvironmentId): DesktopSshEnvironmentConnectionReadResult;
	openTerminalHost(environmentId: EnvironmentId): Promise<SshTerminalHostConnectionOutcome>;
	subscribe(
		environmentId: EnvironmentId,
		listener: (observation: SshEnvironmentConnectionObservation) => void,
	): (() => void) | null;
}

export class DesktopSshEnvironmentConnectionOwner implements DesktopSshEnvironmentConnectionOwnerPort {
	readonly #createSupervisor: DesktopSshEnvironmentConnectionOwnerOptions['createSupervisor'];
	#disposed = false;
	readonly #interactionLeases = new Map<
		EnvironmentId,
		{ readonly interaction: DesktopSshEnvironmentInteraction }
	>();
	readonly #operationTails = new Map<EnvironmentId, Promise<void>>();
	readonly #registry: DesktopSshEnvironmentConnectionOwnerOptions['registry'];
	readonly #supervisors = new Map<EnvironmentId, DesktopSshEnvironmentSupervisorPort>();
	readonly #supported: boolean;

	constructor(options: DesktopSshEnvironmentConnectionOwnerOptions) {
		this.#createSupervisor = options.createSupervisor;
		this.#registry = options.registry;
		this.#supported = options.supported;
	}

	claimInteractionLease(
		environmentId: EnvironmentId,
		interaction: DesktopSshEnvironmentInteraction,
	): DesktopSshEnvironmentInteractionLeaseResult {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (this.#disposed) {
			return { kind: 'unavailable', reason: 'connection-owner-disposed' };
		}
		if (this.#interactionLeases.has(canonicalId)) return { kind: 'busy' };
		const leaseOwner = { interaction };
		this.#interactionLeases.set(canonicalId, leaseOwner);
		let released = false;
		return {
			kind: 'claimed',
			lease: {
				release: () => {
					if (released) return;
					released = true;
					if (this.#interactionLeases.get(canonicalId) === leaseOwner) {
						this.#interactionLeases.delete(canonicalId);
					}
				},
			},
		};
	}

	inspectProjectRoot(
		environmentId: EnvironmentId,
		path: string,
		signal?: AbortSignal,
	): Promise<SshProjectRootInspectionOutcome> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (this.#disposed) return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		const supervisor = this.#supervisors.get(canonicalId);
		return (
			supervisor?.inspectProjectRoot(path, signal) ??
			Promise.resolve({ kind: 'failed', reason: 'unavailable' })
		);
	}

	listProjectDirectory(
		environmentId: EnvironmentId,
		path: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (this.#disposed) return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		const supervisor = this.#supervisors.get(canonicalId);
		return (
			supervisor?.listProjectDirectory(path, signal) ??
			Promise.resolve({ kind: 'failed', reason: 'unavailable' })
		);
	}

	openTerminalHost(environmentId: EnvironmentId): Promise<SshTerminalHostConnectionOutcome> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (this.#disposed) return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		const supervisor = this.#supervisors.get(canonicalId);
		return (
			supervisor?.openTerminalHost() ?? Promise.resolve({ kind: 'failed', reason: 'unavailable' })
		);
	}

	connect(environmentId: EnvironmentId): Promise<DesktopSshEnvironmentConnectResult> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#enqueue(canonicalId, async () => {
			if (this.#disposed) {
				return { kind: 'unavailable', reason: 'connection-owner-disposed' };
			}
			if (!this.#supported) {
				return { kind: 'unavailable', reason: 'local-platform-unsupported' };
			}
			const existing = this.#supervisors.get(canonicalId);
			if (existing !== undefined) {
				existing.start();
				return {
					activation: 'existing',
					kind: 'ready',
					observation: existing.observation(),
				};
			}
			const record = await this.#registry.get(canonicalId);
			if (this.#disposed) {
				return { kind: 'unavailable', reason: 'connection-owner-disposed' };
			}
			if (record.kind === 'unavailable') {
				return { kind: 'unavailable', reason: 'registry-unavailable' };
			}
			if (record.value === null) return { kind: 'not-found' };
			const supervisor = this.#createSupervisor(
				{
					connectionId: record.value.environmentId,
					runtimeInstanceId: record.value.provider.runtimeInstanceId,
					target: record.value.provider.target,
				},
				(request) => this.#requestInteraction(canonicalId, request),
			);
			this.#supervisors.set(canonicalId, supervisor);
			supervisor.start();
			return {
				activation: 'started',
				kind: 'ready',
				observation: supervisor.observation(),
			};
		});
	}

	disconnect(environmentId: EnvironmentId): Promise<DesktopSshEnvironmentDisconnectResult> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#enqueue(canonicalId, async () => {
			if (this.#disposed) {
				return { kind: 'unavailable', reason: 'connection-owner-disposed' };
			}
			const supervisor = this.#supervisors.get(canonicalId);
			if (supervisor === undefined) return { kind: 'inactive' };
			await supervisor.stop();
			if (this.#supervisors.get(canonicalId) === supervisor) {
				this.#supervisors.delete(canonicalId);
			}
			return { kind: 'disconnected' };
		});
	}

	observation(environmentId: EnvironmentId): DesktopSshEnvironmentConnectionReadResult {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		const supervisor = this.#supervisors.get(canonicalId);
		return supervisor === undefined
			? { kind: 'inactive' }
			: { kind: 'ready', observation: supervisor.observation() };
	}

	subscribe(
		environmentId: EnvironmentId,
		listener: (observation: SshEnvironmentConnectionObservation) => void,
	): (() => void) | null {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#supervisors.get(canonicalId)?.subscribe(listener) ?? null;
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#interactionLeases.clear();
		await Promise.all(this.#operationTails.values());
		const supervisors = [...this.#supervisors.values()];
		const settlements = await Promise.allSettled(
			supervisors.map(async (supervisor) => supervisor.stop()),
		);
		this.#supervisors.clear();
		const failures = settlements.flatMap((settlement) =>
			settlement.status === 'rejected' ? [settlement.reason] : [],
		);
		if (failures.length > 0) {
			throw new AggregateError(failures, 'SSH Environment connection cleanup did not converge.');
		}
	}

	#requestInteraction(
		environmentId: EnvironmentId,
		request: SshEnvironmentSupervisorInteractionRequest,
	): Promise<SshAskPassInteractionResult> {
		return (
			this.#interactionLeases.get(environmentId)?.interaction(request) ??
			Promise.resolve({ kind: 'cancelled' })
		);
	}

	#enqueue<Value>(environmentId: EnvironmentId, operation: () => Promise<Value>): Promise<Value> {
		const previous = this.#operationTails.get(environmentId) ?? Promise.resolve();
		const result = previous.then(operation);
		const tail = result.then(
			() => undefined,
			() => undefined,
		);
		this.#operationTails.set(environmentId, tail);
		void tail.then(() => {
			if (this.#operationTails.get(environmentId) === tail) {
				this.#operationTails.delete(environmentId);
			}
		});
		return result;
	}
}
