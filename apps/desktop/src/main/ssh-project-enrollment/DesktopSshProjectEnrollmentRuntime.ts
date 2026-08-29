import {
	DesktopSshProjectEnrollmentInputSchema,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';
import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';

import {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	sshEnvironmentTargetKey,
	type DesktopEnvironmentRegistry,
	type EnvironmentId,
	type EnvironmentRecord,
} from '../environment';
import type { DesktopSshEnvironmentRemoteProjectAdmissionResult } from '../remote-project';
import {
	SshEnvironmentTargetReferenceSchema,
	type OpenSshTargetResolutionResult,
	type SshAskPassInteractionResult,
	type SshEnvironmentConnectionObservation,
	type SshEnvironmentSupervisorInteractionRequest,
	type SshEnvironmentTargetReference,
	type SshProjectRootInspectionOutcome,
} from '../ssh-environment';

export type DesktopSshProjectEnrollmentInteractionRequest =
	SshEnvironmentSupervisorInteractionRequest;

export interface DesktopSshProjectEnrollmentOperation {
	interact(
		request: DesktopSshProjectEnrollmentInteractionRequest,
	): Promise<DesktopSshProjectInteractionResult>;
	progress(stage: DesktopSshProjectEnrollmentProgressStage): void;
}

type DesktopSshProjectEnrollmentProgressStage =
	| 'committing-project'
	| 'connecting'
	| 'inspecting-project'
	| 'recording-environment'
	| 'resolving-target';

interface ConnectionLease {
	release(): void;
}

interface DesktopSshProjectEnrollmentConnectionPort {
	claimInteractionLease(
		environmentId: EnvironmentId,
		interaction: (
			request: SshEnvironmentSupervisorInteractionRequest,
		) => Promise<SshAskPassInteractionResult>,
	):
		| { readonly kind: 'claimed'; readonly lease: ConnectionLease }
		| { readonly kind: 'busy' }
		| { readonly kind: 'unavailable'; readonly reason: 'connection-owner-disposed' };
	connect(environmentId: EnvironmentId): Promise<
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
		  }
	>;
	disconnect(
		environmentId: EnvironmentId,
	): Promise<
		| { readonly kind: 'disconnected' | 'inactive' }
		| { readonly kind: 'unavailable'; readonly reason: 'connection-owner-disposed' }
	>;
	inspectProjectRoot(
		environmentId: EnvironmentId,
		path: string,
		signal?: AbortSignal,
	): Promise<SshProjectRootInspectionOutcome>;
	observation(
		environmentId: EnvironmentId,
	):
		| { readonly kind: 'inactive' }
		| { readonly kind: 'ready'; readonly observation: SshEnvironmentConnectionObservation };
	subscribe(
		environmentId: EnvironmentId,
		listener: (observation: SshEnvironmentConnectionObservation) => void,
	): (() => void) | null;
}

export interface DesktopSshProjectEnrollmentRuntimeOptions {
	readonly admitProject: (
		environmentId: EnvironmentId,
		requestedPath: string,
		signal?: AbortSignal,
	) => Promise<DesktopSshEnvironmentRemoteProjectAdmissionResult>;
	readonly allocateIdentifier: () => string;
	readonly connections: DesktopSshProjectEnrollmentConnectionPort;
	readonly getDocument: () => WorkbenchSharedDocument | null;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'create' | 'list'>;
	readonly resolveTarget: (
		target: SshEnvironmentTargetReference,
	) => Promise<OpenSshTargetResolutionResult>;
}

type StatusListener = (statuses: readonly DesktopSshProjectConnectionStatus[]) => void;

export class DesktopSshProjectEnrollmentRuntime {
	readonly #activeEnvironments = new Set<EnvironmentId>();
	readonly #admitProject: DesktopSshProjectEnrollmentRuntimeOptions['admitProject'];
	readonly #allocateIdentifier: () => string;
	readonly #connections: DesktopSshProjectEnrollmentConnectionPort;
	readonly #environmentSubscriptions = new Map<EnvironmentId, () => void>();
	readonly #getDocument: () => WorkbenchSharedDocument | null;
	readonly #listeners = new Set<StatusListener>();
	readonly #registry: DesktopSshProjectEnrollmentRuntimeOptions['registry'];
	readonly #resolveTarget: DesktopSshProjectEnrollmentRuntimeOptions['resolveTarget'];

	constructor(options: DesktopSshProjectEnrollmentRuntimeOptions) {
		this.#admitProject = options.admitProject;
		this.#allocateIdentifier = options.allocateIdentifier;
		this.#connections = options.connections;
		this.#getDocument = options.getDocument;
		this.#registry = options.registry;
		this.#resolveTarget = options.resolveTarget;
	}

	async enroll(
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectEnrollmentResult> {
		const canonicalInput = DesktopSshProjectEnrollmentInputSchema.parse(input);
		if (signal.aborted) return { kind: 'cancelled' };
		const target = SshEnvironmentTargetReferenceSchema.parse({
			configurationFilePath: canonicalInput.configurationFilePath,
			destination: canonicalInput.destination,
		});
		operation.progress('resolving-target');
		const resolution = await this.#resolveTarget(target).catch(() => null);
		if (signal.aborted) return { kind: 'cancelled' };
		if (resolution === null || resolution.kind === 'failed') {
			return { kind: 'failed', reason: 'target-resolution-failed' };
		}

		operation.progress('recording-environment');
		const environmentResult = await this.#recordEnvironment(canonicalInput.name, target);
		if (environmentResult.kind !== 'ready') return environmentResult.result;
		const environmentId = environmentResult.environment.environmentId;
		if (signal.aborted) return { kind: 'cancelled' };
		if (!this.#beginEnvironmentOperation(environmentId)) {
			return { kind: 'failed', reason: 'busy' };
		}

		let activation: 'existing' | 'started' | undefined;
		let commitMayHaveHappened = false;
		let lease: ConnectionLease | undefined;
		try {
			const claim = this.#connections.claimInteractionLease(environmentId, (request) =>
				operation.interact(request),
			);
			if (claim.kind === 'busy') return { kind: 'failed', reason: 'busy' };
			if (claim.kind === 'unavailable') {
				return { kind: 'failed', reason: 'connection-owner-unavailable' };
			}
			lease = claim.lease;
			operation.progress('connecting');
			const connected = await this.#connect(environmentId, signal);
			activation = connected.activation;
			if (connected.kind !== 'connected') return connected.result;

			operation.progress('inspecting-project');
			const inspected = await this.#connections.inspectProjectRoot(
				environmentId,
				canonicalInput.projectPath,
				signal,
			);
			if (signal.aborted || inspected.kind === 'cancelled') return { kind: 'cancelled' };
			if (inspected.kind === 'failed') return mapProjectRootFailure(inspected.reason);

			operation.progress('committing-project');
			commitMayHaveHappened = true;
			const admitted = await this.#admitProject(environmentId, inspected.canonicalPath, signal);
			return mapAdmission(admitted);
		} finally {
			lease?.release();
			this.#activeEnvironments.delete(environmentId);
			if (
				activation === 'started' &&
				!commitMayHaveHappened &&
				!this.#environmentIsReferenced(environmentId)
			) {
				await this.#connections.disconnect(environmentId).catch(() => undefined);
				this.#environmentSubscriptions.get(environmentId)?.();
				this.#environmentSubscriptions.delete(environmentId);
			}
			this.#publishStatuses();
		}
	}

	async connectProject(
		projectId: string,
		operation: Pick<DesktopSshProjectEnrollmentOperation, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult> {
		const target = this.#resolveProjectEnvironment(projectId);
		if (target.kind !== 'resolved') return target.result;
		if (signal.aborted) return { kind: 'cancelled' };
		if (!this.#beginEnvironmentOperation(target.environmentId)) {
			return { kind: 'failed', reason: 'busy' };
		}
		let lease: ConnectionLease | undefined;
		try {
			const claim = this.#connections.claimInteractionLease(target.environmentId, (request) =>
				operation.interact(request),
			);
			if (claim.kind === 'busy') return { kind: 'failed', reason: 'busy' };
			if (claim.kind === 'unavailable') {
				return { kind: 'failed', reason: 'connection-owner-unavailable' };
			}
			lease = claim.lease;
			const connected = await this.#connect(target.environmentId, signal);
			return connected.kind === 'connected'
				? { kind: 'connected', projectId }
				: mapEnrollmentConnectionFailure(connected.result);
		} finally {
			lease?.release();
			this.#activeEnvironments.delete(target.environmentId);
			this.#publishStatuses();
		}
	}

	async disconnectProject(
		projectId: string,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult> {
		const target = this.#resolveProjectEnvironment(projectId);
		if (target.kind !== 'resolved') return target.result;
		if (signal.aborted) return { kind: 'cancelled' };
		if (!this.#beginEnvironmentOperation(target.environmentId)) {
			return { kind: 'failed', reason: 'busy' };
		}
		try {
			const disconnected = await this.#connections.disconnect(target.environmentId);
			if (disconnected.kind === 'unavailable') {
				return { kind: 'failed', reason: 'connection-owner-unavailable' };
			}
			this.#environmentSubscriptions.get(target.environmentId)?.();
			this.#environmentSubscriptions.delete(target.environmentId);
			return { kind: 'disconnected', projectId };
		} catch {
			return { kind: 'failed', reason: 'connection-owner-unavailable' };
		} finally {
			this.#activeEnvironments.delete(target.environmentId);
			this.#publishStatuses();
		}
	}

	statuses(): readonly DesktopSshProjectConnectionStatus[] {
		const document = this.#getDocument();
		if (document === null) return [];
		return document.projectWorkspace.projects.flatMap((project) => {
			const environmentIds = new Set(
				project.directories.flatMap((directory) =>
					directory.placement.kind === 'environment' ? [directory.placement.environmentId] : [],
				),
			);
			if (environmentIds.size === 0) return [];
			const environmentId = EnvironmentIdSchema.safeParse([...environmentIds][0]);
			if (!environmentId.success || environmentIds.size !== 1) {
				return [{ blockedReason: null, projectId: project.id, state: 'unavailable' as const }];
			}
			const connection = this.#connections.observation(environmentId.data);
			return [
				connection.kind === 'inactive'
					? { blockedReason: null, projectId: project.id, state: 'disconnected' as const }
					: statusFromObservation(project.id, connection.observation),
			];
		});
	}

	subscribeStatuses(listener: StatusListener): () => void {
		this.#listeners.add(listener);
		listener(this.statuses());
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		for (const unsubscribe of this.#environmentSubscriptions.values()) unsubscribe();
		this.#environmentSubscriptions.clear();
		this.#listeners.clear();
	}

	async #recordEnvironment(
		name: string,
		target: EnvironmentRecord['provider']['target'],
	): Promise<
		| { readonly environment: EnvironmentRecord; readonly kind: 'ready' }
		| { readonly kind: 'settled'; readonly result: DesktopSshProjectEnrollmentResult }
	> {
		for (let attempt = 0; attempt < 4; attempt += 1) {
			const current = await this.#registry.list();
			if (current.kind === 'unavailable') {
				return { kind: 'settled', result: { kind: 'failed', reason: 'registry-unavailable' } };
			}
			const key = sshEnvironmentTargetKey(target);
			const existing = current.value.find(
				(environment) => sshEnvironmentTargetKey(environment.provider.target) === key,
			);
			if (existing !== undefined) return { environment: existing, kind: 'ready' };
			const environment = EnvironmentRecordSchema.parse({
				environmentId: `environment:${this.#allocateIdentifier()}`,
				name,
				provider: {
					kind: 'ssh',
					runtimeInstanceId: this.#allocateIdentifier(),
					target,
				},
			});
			const result = await this.#registry.create(environment);
			switch (result.kind) {
				case 'created':
				case 'existing':
					return { environment: result.environment, kind: 'ready' };
				case 'conflict':
					if (result.reason === 'ssh-target') {
						return { environment: result.environment, kind: 'ready' };
					}
					continue;
				case 'indeterminate':
					return {
						kind: 'settled',
						result: { kind: 'indeterminate', reason: 'durability-unconfirmed' },
					};
				case 'rejected':
					return { kind: 'settled', result: { kind: 'failed', reason: 'environment-capacity' } };
				case 'unavailable':
					return { kind: 'settled', result: { kind: 'failed', reason: 'registry-unavailable' } };
			}
		}
		return { kind: 'settled', result: { kind: 'failed', reason: 'environment-conflict' } };
	}

	async #connect(
		environmentId: EnvironmentId,
		signal: AbortSignal,
	): Promise<
		| { readonly activation: 'existing' | 'started'; readonly kind: 'connected' }
		| {
				readonly activation?: 'existing' | 'started';
				readonly kind: 'settled';
				readonly result: DesktopSshProjectEnrollmentResult;
		  }
	> {
		const activation = await this.#connections.connect(environmentId);
		if (activation.kind === 'not-found') {
			return { kind: 'settled', result: { kind: 'failed', reason: 'environment-not-found' } };
		}
		if (activation.kind === 'unavailable') {
			let reason: Extract<
				DesktopSshProjectEnrollmentResult,
				{ readonly kind: 'failed' }
			>['reason'] = 'connection-owner-unavailable';
			if (activation.reason === 'local-platform-unsupported') {
				reason = 'local-platform-unsupported';
			} else if (activation.reason === 'registry-unavailable') {
				reason = 'registry-unavailable';
			}
			return {
				kind: 'settled',
				result: { kind: 'failed', reason },
			};
		}
		this.#bindObservation(environmentId);
		const observation = await waitForConnection(
			this.#connections,
			environmentId,
			activation.observation,
			signal,
		);
		if (observation === 'cancelled') {
			return { activation: activation.activation, kind: 'settled', result: { kind: 'cancelled' } };
		}
		if (!observation.connected) {
			return {
				activation: activation.activation,
				kind: 'settled',
				result: { kind: 'failed', reason: 'remote-runtime-unavailable' },
			};
		}
		return { activation: activation.activation, kind: 'connected' };
	}

	#bindObservation(environmentId: EnvironmentId): void {
		this.#environmentSubscriptions.get(environmentId)?.();
		const unsubscribe = this.#connections.subscribe(environmentId, () => this.#publishStatuses());
		if (unsubscribe === null) this.#environmentSubscriptions.delete(environmentId);
		else this.#environmentSubscriptions.set(environmentId, unsubscribe);
	}

	#beginEnvironmentOperation(environmentId: EnvironmentId): boolean {
		if (this.#activeEnvironments.has(environmentId)) return false;
		this.#activeEnvironments.add(environmentId);
		return true;
	}

	#environmentIsReferenced(environmentId: EnvironmentId): boolean {
		return (
			this.#getDocument()?.projectWorkspace.projects.some((project) =>
				project.directories.some(
					(directory) =>
						directory.placement.kind === 'environment' &&
						directory.placement.environmentId === environmentId,
				),
			) ?? false
		);
	}

	#publishStatuses(): void {
		const statuses = this.statuses();
		for (const listener of this.#listeners) listener(statuses);
	}

	#resolveProjectEnvironment(
		projectId: string,
	):
		| { readonly environmentId: EnvironmentId; readonly kind: 'resolved' }
		| { readonly kind: 'settled'; readonly result: DesktopSshProjectConnectionResult } {
		const document = this.#getDocument();
		if (document === null) {
			return { kind: 'settled', result: { kind: 'failed', reason: 'workbench-missing' } };
		}
		const project = document.projectWorkspace.projects.find(
			(candidate) => candidate.id === projectId,
		);
		if (project === undefined) {
			return { kind: 'settled', result: { kind: 'failed', reason: 'project-not-found' } };
		}
		const environmentIds = new Set(
			project.directories.flatMap((directory) =>
				directory.placement.kind === 'environment' ? [directory.placement.environmentId] : [],
			),
		);
		const parsed = EnvironmentIdSchema.safeParse([...environmentIds][0]);
		if (environmentIds.size !== 1 || !parsed.success) {
			return { kind: 'settled', result: { kind: 'failed', reason: 'environment-not-found' } };
		}
		return { environmentId: parsed.data, kind: 'resolved' };
	}
}

async function waitForConnection(
	connections: DesktopSshProjectEnrollmentConnectionPort,
	environmentId: EnvironmentId,
	initial: SshEnvironmentConnectionObservation,
	signal: AbortSignal,
): Promise<'cancelled' | SshEnvironmentConnectionObservation> {
	if (signal.aborted) return 'cancelled';
	const immediate = terminalConnectionObservation(initial);
	if (immediate !== null) return immediate;
	return new Promise((resolve) => {
		let settled = false;
		let unsubscribe: (() => void) | null = null;
		const settle = (value: 'cancelled' | SshEnvironmentConnectionObservation): void => {
			if (settled) return;
			settled = true;
			signal.removeEventListener('abort', abort);
			unsubscribe?.();
			resolve(value);
		};
		const abort = (): void => settle('cancelled');
		signal.addEventListener('abort', abort, { once: true });
		unsubscribe = connections.subscribe(environmentId, (observation) => {
			const terminal = terminalConnectionObservation(observation);
			if (terminal !== null) settle(terminal);
		});
		if (settled) unsubscribe?.();
		if (unsubscribe === null) settle(initial);
		else if (signal.aborted) settle('cancelled');
	});
}

function terminalConnectionObservation(
	observation: SshEnvironmentConnectionObservation,
): SshEnvironmentConnectionObservation | null {
	return observation.phase === 'connected' ||
		observation.phase === 'blocked' ||
		observation.phase === 'stopped'
		? observation
		: null;
}

function statusFromObservation(
	projectId: string,
	observation: SshEnvironmentConnectionObservation,
): DesktopSshProjectConnectionStatus {
	const state = (() => {
		switch (observation.phase) {
			case 'blocked':
				return 'blocked';
			case 'connected':
				return 'connected';
			case 'interaction-required':
				return 'interaction-required';
			case 'retry-wait':
				return 'retrying';
			case 'connecting':
			case 'resolving':
				return 'connecting';
			case 'stopped':
				return 'disconnected';
		}
	})();
	return { blockedReason: observation.blockedReason, projectId, state };
}

function mapProjectRootFailure(
	reason: Extract<SshProjectRootInspectionOutcome, { readonly kind: 'failed' }>['reason'],
): DesktopSshProjectEnrollmentResult {
	const mapped = {
		busy: 'project-root-busy',
		'invalid-path': 'project-root-invalid',
		'not-directory': 'project-root-not-directory',
		'not-found': 'project-root-not-found',
		'permission-denied': 'project-root-permission-denied',
		'timed-out': 'project-root-timed-out',
		unavailable: 'remote-runtime-unavailable',
	} as const;
	return { kind: 'failed', reason: mapped[reason] };
}

function mapAdmission(
	result: DesktopSshEnvironmentRemoteProjectAdmissionResult,
): DesktopSshProjectEnrollmentResult {
	if ('canonicalPath' in result) {
		return {
			canonicalPath: result.canonicalPath,
			kind: result.kind,
			projectId: result.projectId,
			workspaceId: result.workspaceId,
		};
	}
	if (result.kind === 'cancelled' || result.kind === 'indeterminate') return result;
	switch (result.reason) {
		case 'workbench-missing':
		case 'workbench-unavailable':
			return { kind: 'failed', reason: result.reason };
		case 'environment-not-found':
			return { kind: 'failed', reason: 'environment-not-found' };
		case 'registry-unavailable':
			return { kind: 'failed', reason: 'registry-unavailable' };
		case 'environment-not-connected':
			return { kind: 'failed', reason: 'remote-runtime-unavailable' };
		case 'busy':
		case 'invalid-path':
		case 'not-directory':
		case 'not-found':
		case 'permission-denied':
		case 'timed-out':
		case 'unavailable':
			return mapProjectRootFailure(result.reason);
	}
}

function mapEnrollmentConnectionFailure(
	result: DesktopSshProjectEnrollmentResult,
): DesktopSshProjectConnectionResult {
	if (result.kind === 'cancelled') return result;
	if (result.kind === 'indeterminate') {
		return { kind: 'indeterminate', reason: 'control-unavailable' };
	}
	if ('canonicalPath' in result) {
		return { kind: 'failed', reason: 'connection-owner-unavailable' };
	}
	switch (result.reason) {
		case 'connection-owner-unavailable':
		case 'environment-not-found':
		case 'local-platform-unsupported':
		case 'registry-unavailable':
		case 'remote-runtime-unavailable':
		case 'workbench-missing':
		case 'workbench-unavailable':
			return { kind: 'failed', reason: result.reason };
		default:
			return { kind: 'failed', reason: 'connection-owner-unavailable' };
	}
}
