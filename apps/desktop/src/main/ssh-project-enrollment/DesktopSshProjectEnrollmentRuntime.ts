import {
	DesktopSshEnvironmentCatalogSchema,
	DesktopSshEnvironmentIdSchema,
	DesktopSshProjectEnrollmentInputSchema,
	type DesktopSshEnvironmentAddResult,
	type DesktopSshEnvironmentCatalog,
	type DesktopSshEnvironmentConnectionResult,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshProjectDirectorySelectionResult,
	type DesktopSshProjectDirectorySnapshot,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentProgressStage,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectFolderAttachmentResult,
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
import type {
	DesktopSshEnvironmentRemoteProjectAdmissionResult,
	DesktopSshEnvironmentRemoteProjectFolderAttachmentResult,
} from '../remote-project';
import {
	SshEnvironmentTargetReferenceSchema,
	formatOpenSshConnectionCommand,
	parseOpenSshConnectionCommand,
	type OpenSshTargetResolutionResult,
	type SshAskPassInteractionResult,
	type SshEnvironmentConnectionObservation,
	type SshEnvironmentSupervisorInteractionRequest,
	type SshEnvironmentTargetReference,
	type SshProjectRootDirectoryListingOutcome,
	type SshProjectRootInspectionOutcome,
} from '../ssh-environment';
import type { OpenSshHostAliasDiscovery } from './discoverOpenSshHostAliases';

export type DesktopSshProjectEnrollmentInteractionRequest =
	SshEnvironmentSupervisorInteractionRequest;

export interface DesktopSshProjectEnrollmentOperation {
	interact(
		request: DesktopSshProjectEnrollmentInteractionRequest,
	): Promise<DesktopSshProjectInteractionResult>;
	progress(stage: DesktopSshProjectEnrollmentProgressStage): void;
	selectDirectory(
		snapshot: DesktopSshProjectDirectorySnapshot,
	): Promise<DesktopSshProjectDirectorySelectionResult>;
}

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
	listProjectDirectory(
		environmentId: EnvironmentId,
		path: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome>;
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
	readonly attachProjectFolder: (
		environmentId: EnvironmentId,
		projectId: string,
		requestedPath: string,
		signal?: AbortSignal,
	) => Promise<DesktopSshEnvironmentRemoteProjectFolderAttachmentResult>;
	readonly allocateIdentifier: () => string;
	readonly connections: DesktopSshProjectEnrollmentConnectionPort;
	readonly discoverHostAliases?: (() => Promise<OpenSshHostAliasDiscovery>) | undefined;
	readonly getDocument: () => WorkbenchSharedDocument | null;
	readonly homeDirectory?: string | undefined;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'create' | 'list'>;
	readonly resolveTarget: (
		target: SshEnvironmentTargetReference,
	) => Promise<OpenSshTargetResolutionResult>;
}

type StatusListener = (statuses: readonly DesktopSshProjectConnectionStatus[]) => void;
type CatalogListener = () => void;
type DesktopSshProjectDirectoryOperationSettlement = Exclude<
	DesktopSshProjectEnrollmentResult,
	{ readonly canonicalPath: string }
>;

export class DesktopSshProjectEnrollmentRuntime {
	readonly #activeEnvironments = new Set<EnvironmentId>();
	readonly #admitProject: DesktopSshProjectEnrollmentRuntimeOptions['admitProject'];
	readonly #allocateIdentifier: () => string;
	readonly #attachProjectFolder: DesktopSshProjectEnrollmentRuntimeOptions['attachProjectFolder'];
	readonly #connections: DesktopSshProjectEnrollmentConnectionPort;
	readonly #discoverHostAliases: () => Promise<OpenSshHostAliasDiscovery>;
	#discovery: Promise<OpenSshHostAliasDiscovery> | undefined;
	readonly #environmentSubscriptions = new Map<EnvironmentId, () => void>();
	readonly #catalogListeners = new Set<CatalogListener>();
	readonly #getDocument: () => WorkbenchSharedDocument | null;
	readonly #homeDirectory: string | undefined;
	readonly #listeners = new Set<StatusListener>();
	readonly #registry: DesktopSshProjectEnrollmentRuntimeOptions['registry'];
	readonly #resolveTarget: DesktopSshProjectEnrollmentRuntimeOptions['resolveTarget'];

	constructor(options: DesktopSshProjectEnrollmentRuntimeOptions) {
		this.#admitProject = options.admitProject;
		this.#allocateIdentifier = options.allocateIdentifier;
		this.#attachProjectFolder = options.attachProjectFolder;
		this.#connections = options.connections;
		this.#discoverHostAliases =
			options.discoverHostAliases ?? (() => Promise.resolve({ aliases: [], issue: null }));
		this.#getDocument = options.getDocument;
		this.#homeDirectory = options.homeDirectory;
		this.#registry = options.registry;
		this.#resolveTarget = options.resolveTarget;
	}

	async environmentCatalog(refreshDiscovery: boolean): Promise<DesktopSshEnvironmentCatalog> {
		const discovery = await this.#readDiscovery(refreshDiscovery);
		const registry = await this.#registry.list();
		if (registry.kind === 'unavailable') {
			return DesktopSshEnvironmentCatalogSchema.parse({
				discoveryIssue: discovery.issue,
				managed: [],
				registryIssue: 'unavailable',
				suggestions: discovery.aliases.map((destination) => ({ destination })),
			});
		}
		const managedTargetKeys = new Set(
			registry.value.map((environment) => sshEnvironmentTargetKey(environment.provider.target)),
		);
		return DesktopSshEnvironmentCatalogSchema.parse({
			discoveryIssue: discovery.issue,
			managed: registry.value
				.map((environment) => ({
					...environmentConnectionStatus(this.#connections.observation(environment.environmentId)),
					destination: formatOpenSshConnectionCommand(environment.provider.target),
					environmentId: DesktopSshEnvironmentIdSchema.parse(environment.environmentId),
					name: environment.name,
					projectCount: this.#environmentProjectCount(environment.environmentId),
				}))
				.sort(compareManagedEnvironments),
			registryIssue: null,
			suggestions: discovery.aliases
				.filter(
					(destination) =>
						!managedTargetKeys.has(
							sshEnvironmentTargetKey(
								SshEnvironmentTargetReferenceSchema.parse({
									configurationFilePath: null,
									destination,
								}),
							),
						),
				)
				.map((destination) => ({ destination })),
		});
	}

	async addEnvironment(
		input: DesktopSshProjectEnrollmentInput,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentAddResult> {
		const canonicalInput = DesktopSshProjectEnrollmentInputSchema.parse(input);
		if (signal.aborted) return { kind: 'cancelled' };
		let target: SshEnvironmentTargetReference;
		try {
			target = parseOpenSshConnectionCommand(canonicalInput.destination, {
				homeDirectory: this.#homeDirectory,
			});
		} catch {
			return { kind: 'failed', reason: 'target-resolution-failed' };
		}
		const resolution = await this.#resolveTarget(target).catch(() => null);
		if (signal.aborted) return { kind: 'cancelled' };
		if (resolution === null || resolution.kind === 'failed') {
			return { kind: 'failed', reason: 'target-resolution-failed' };
		}
		const recorded = await this.#recordEnvironment(
			deriveEnvironmentName(formatOpenSshConnectionCommand(target)),
			target,
		);
		if (recorded.kind === 'ready') {
			this.#publishCatalog();
			return {
				environmentId: DesktopSshEnvironmentIdSchema.parse(recorded.environment.environmentId),
				kind: recorded.registration === 'created' ? 'added' : 'existing',
			};
		}
		if (recorded.result.kind === 'cancelled') return recorded.result;
		if (recorded.result.kind === 'indeterminate') return recorded.result;
		switch (recorded.result.reason) {
			case 'environment-capacity':
			case 'environment-conflict':
			case 'registry-unavailable':
				return { kind: 'failed', reason: recorded.result.reason };
			default:
				return { kind: 'failed', reason: 'registry-unavailable' };
		}
	}

	async connectEnvironment(
		environmentId: string,
		operation: Pick<DesktopSshProjectEnrollmentOperation, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (signal.aborted) return { kind: 'cancelled' };
		if (!this.#beginEnvironmentOperation(canonicalId)) {
			return { kind: 'failed', reason: 'busy' };
		}
		let lease: ConnectionLease | undefined;
		try {
			const claim = this.#connections.claimInteractionLease(canonicalId, (request) =>
				operation.interact(request),
			);
			if (claim.kind === 'busy') return { kind: 'failed', reason: 'busy' };
			if (claim.kind === 'unavailable') {
				return { kind: 'failed', reason: 'connection-owner-unavailable' };
			}
			lease = claim.lease;
			const connected = await this.#connect(canonicalId, signal);
			if (connected.kind === 'connected') {
				return {
					environmentId: DesktopSshEnvironmentIdSchema.parse(canonicalId),
					kind: 'connected',
				};
			}
			return mapEnvironmentConnectionFailure(connected.result);
		} finally {
			lease?.release();
			this.#activeEnvironments.delete(canonicalId);
			this.#publishStatuses();
			this.#publishCatalog();
		}
	}

	async disconnectEnvironment(
		environmentId: string,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		if (signal.aborted) return { kind: 'cancelled' };
		if (!this.#beginEnvironmentOperation(canonicalId)) {
			return { kind: 'failed', reason: 'busy' };
		}
		try {
			const disconnected = await this.#connections.disconnect(canonicalId);
			if (disconnected.kind === 'unavailable') {
				return { kind: 'failed', reason: 'connection-owner-unavailable' };
			}
			this.#environmentSubscriptions.get(canonicalId)?.();
			this.#environmentSubscriptions.delete(canonicalId);
			return {
				environmentId: DesktopSshEnvironmentIdSchema.parse(canonicalId),
				kind: 'disconnected',
			};
		} catch {
			return { kind: 'failed', reason: 'connection-owner-unavailable' };
		} finally {
			this.#activeEnvironments.delete(canonicalId);
			this.#publishStatuses();
			this.#publishCatalog();
		}
	}

	subscribeEnvironmentCatalog(listener: CatalogListener): () => void {
		this.#catalogListeners.add(listener);
		return () => this.#catalogListeners.delete(listener);
	}

	async enroll(
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectEnrollmentResult> {
		return this.#runDirectoryOperation(
			input,
			operation,
			signal,
			'committing-project',
			(result) => result,
			async (environmentId, canonicalPath) =>
				mapAdmission(await this.#admitProject(environmentId, canonicalPath, signal)),
		);
	}

	async attachFolder(
		projectId: string,
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectFolderAttachmentResult> {
		return this.#runDirectoryOperation(
			input,
			operation,
			signal,
			'committing-project-folder',
			(result) => result,
			async (environmentId, canonicalPath) =>
				mapFolderAttachment(
					await this.#attachProjectFolder(environmentId, projectId, canonicalPath, signal),
				),
		);
	}

	async #runDirectoryOperation<Result extends { readonly kind: string }>(
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
		commitStage: 'committing-project' | 'committing-project-folder',
		mapSettlement: (result: DesktopSshProjectDirectoryOperationSettlement) => Result,
		commit: (environmentId: EnvironmentId, canonicalPath: string) => Promise<Result>,
	): Promise<Result> {
		const canonicalInput = DesktopSshProjectEnrollmentInputSchema.parse(input);
		if (signal.aborted) return mapSettlement({ kind: 'cancelled' });
		let target: SshEnvironmentTargetReference;
		try {
			target = parseOpenSshConnectionCommand(canonicalInput.destination, {
				homeDirectory: this.#homeDirectory,
			});
		} catch {
			return mapSettlement({ kind: 'failed', reason: 'target-resolution-failed' });
		}
		operation.progress('resolving-target');
		const resolution = await this.#resolveTarget(target).catch(() => null);
		if (signal.aborted) return mapSettlement({ kind: 'cancelled' });
		if (resolution === null || resolution.kind === 'failed') {
			return mapSettlement({ kind: 'failed', reason: 'target-resolution-failed' });
		}

		operation.progress('recording-environment');
		const environmentResult = await this.#recordEnvironment(
			deriveEnvironmentName(formatOpenSshConnectionCommand(target)),
			target,
		);
		if (environmentResult.kind !== 'ready') return mapSettlement(environmentResult.result);
		this.#publishCatalog();
		const environmentId = environmentResult.environment.environmentId;
		if (signal.aborted) return mapSettlement({ kind: 'cancelled' });
		if (!this.#beginEnvironmentOperation(environmentId)) {
			return mapSettlement({ kind: 'failed', reason: 'busy' });
		}

		let activation: 'existing' | 'started' | undefined;
		let commitMayHaveHappened = false;
		let lease: ConnectionLease | undefined;
		try {
			const claim = this.#connections.claimInteractionLease(environmentId, (request) =>
				operation.interact(request),
			);
			if (claim.kind === 'busy') return mapSettlement({ kind: 'failed', reason: 'busy' });
			if (claim.kind === 'unavailable') {
				return mapSettlement({ kind: 'failed', reason: 'connection-owner-unavailable' });
			}
			lease = claim.lease;
			operation.progress('connecting');
			const connected = await this.#connect(environmentId, signal);
			activation = connected.activation;
			if (connected.kind !== 'connected') return mapSettlement(connected.result);

			operation.progress('selecting-project-directory');
			const selectedPath = await this.#selectProjectDirectory(environmentId, operation, signal);
			if (selectedPath.kind !== 'selected') return mapSettlement(selectedPath.result);

			operation.progress('inspecting-project');
			const inspected = await this.#connections.inspectProjectRoot(
				environmentId,
				selectedPath.path,
				signal,
			);
			if (signal.aborted || inspected.kind === 'cancelled') {
				return mapSettlement({ kind: 'cancelled' });
			}
			if (inspected.kind === 'failed') {
				return mapSettlement(mapProjectRootFailure(inspected.reason));
			}

			operation.progress(commitStage);
			commitMayHaveHappened = true;
			const result = await commit(environmentId, inspected.canonicalPath);
			return result;
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

	async #selectProjectDirectory(
		environmentId: EnvironmentId,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<
		| { readonly kind: 'selected'; readonly path: string }
		| { readonly kind: 'settled'; readonly result: DesktopSshProjectDirectoryOperationSettlement }
	> {
		const initial = await this.#connections.listProjectDirectory(environmentId, null, signal);
		if (signal.aborted || initial.kind === 'cancelled') {
			return { kind: 'settled', result: { kind: 'cancelled' } };
		}
		if (initial.kind === 'failed') {
			return { kind: 'settled', result: mapProjectRootFailure(initial.reason) };
		}
		let snapshot = directorySnapshot(initial, null);
		while (!signal.aborted) {
			const selection = await operation.selectDirectory(snapshot);
			if (signal.aborted || selection.kind === 'cancelled') {
				return { kind: 'settled', result: { kind: 'cancelled' } };
			}
			if (selection.kind === 'selected') {
				return { kind: 'selected', path: snapshot.canonicalPath };
			}
			const listed = await this.#connections.listProjectDirectory(
				environmentId,
				selection.path,
				signal,
			);
			if (signal.aborted || listed.kind === 'cancelled') {
				return { kind: 'settled', result: { kind: 'cancelled' } };
			}
			snapshot =
				listed.kind === 'failed'
					? { ...snapshot, navigationFailure: listed.reason }
					: directorySnapshot(listed, null);
		}
		return { kind: 'settled', result: { kind: 'cancelled' } };
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
			this.#publishCatalog();
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
			this.#publishCatalog();
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
		this.#catalogListeners.clear();
	}

	async #recordEnvironment(
		name: string,
		target: EnvironmentRecord['provider']['target'],
	): Promise<
		| {
				readonly environment: EnvironmentRecord;
				readonly kind: 'ready';
				readonly registration: 'created' | 'existing';
		  }
		| { readonly kind: 'settled'; readonly result: DesktopSshProjectDirectoryOperationSettlement }
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
			if (existing !== undefined) {
				return { environment: existing, kind: 'ready', registration: 'existing' };
			}
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
					return {
						environment: result.environment,
						kind: 'ready',
						registration: 'created',
					};
				case 'existing':
					return {
						environment: result.environment,
						kind: 'ready',
						registration: 'existing',
					};
				case 'conflict':
					if (result.reason === 'ssh-target') {
						return {
							environment: result.environment,
							kind: 'ready',
							registration: 'existing',
						};
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
				readonly result: DesktopSshProjectDirectoryOperationSettlement;
		  }
	> {
		const activation = await this.#connections.connect(environmentId);
		if (activation.kind === 'not-found') {
			return { kind: 'settled', result: { kind: 'failed', reason: 'environment-not-found' } };
		}
		if (activation.kind === 'unavailable') {
			let reason: Extract<
				DesktopSshProjectDirectoryOperationSettlement,
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
		const unsubscribe = this.#connections.subscribe(environmentId, () => {
			this.#publishStatuses();
			this.#publishCatalog();
		});
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

	#publishCatalog(): void {
		for (const listener of this.#catalogListeners) listener();
	}

	#environmentProjectCount(environmentId: EnvironmentId): number {
		return (
			this.#getDocument()?.projectWorkspace.projects.filter((project) =>
				project.directories.some(
					(directory) =>
						directory.placement.kind === 'environment' &&
						directory.placement.environmentId === environmentId,
				),
			).length ?? 0
		);
	}

	#readDiscovery(refresh: boolean): Promise<OpenSshHostAliasDiscovery> {
		if (refresh || this.#discovery === undefined) {
			this.#discovery = this.#discoverHostAliases().catch(() => ({
				aliases: [],
				issue: 'read-failed',
			}));
		}
		return this.#discovery;
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

function deriveEnvironmentName(destination: string): string {
	let name = destination.slice(0, 256);
	const finalCodeUnit = name.charCodeAt(name.length - 1);
	if (finalCodeUnit >= 0xd800 && finalCodeUnit <= 0xdbff) name = name.slice(0, -1);
	return name;
}

function directorySnapshot(
	listing: Extract<SshProjectRootDirectoryListingOutcome, { readonly kind: 'listed' }>,
	navigationFailure: DesktopSshProjectDirectorySnapshot['navigationFailure'],
): DesktopSshProjectDirectorySnapshot {
	return {
		canonicalPath: listing.canonicalPath,
		directories: listing.directories,
		navigationFailure,
		parentPath: listing.parentPath,
		truncated: listing.truncated,
	};
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
	return { ...connectionStatusFromObservation(observation), projectId };
}

function connectionStatusFromObservation(observation: SshEnvironmentConnectionObservation): {
	readonly blockedReason: SshEnvironmentConnectionObservation['blockedReason'];
	readonly state: DesktopSshEnvironmentCatalog['managed'][number]['state'];
} {
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
	return { blockedReason: observation.blockedReason, state };
}

function environmentConnectionStatus(
	connection: ReturnType<DesktopSshProjectEnrollmentConnectionPort['observation']>,
): Pick<DesktopSshEnvironmentCatalog['managed'][number], 'blockedReason' | 'state'> {
	return connection.kind === 'inactive'
		? { blockedReason: null, state: 'disconnected' }
		: connectionStatusFromObservation(connection.observation);
}

function compareManagedEnvironments(
	left: DesktopSshEnvironmentCatalog['managed'][number],
	right: DesktopSshEnvironmentCatalog['managed'][number],
): number {
	const nameOrder = left.name.toLowerCase().localeCompare(right.name.toLowerCase(), 'en-US');
	if (nameOrder !== 0) return nameOrder;
	return left.destination.localeCompare(right.destination, 'en-US');
}

function mapProjectRootFailure(
	reason: Extract<SshProjectRootInspectionOutcome, { readonly kind: 'failed' }>['reason'],
): DesktopSshProjectDirectoryOperationSettlement {
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

function mapFolderAttachment(
	result: DesktopSshEnvironmentRemoteProjectFolderAttachmentResult,
): DesktopSshProjectFolderAttachmentResult {
	if ('canonicalPath' in result) return result;
	if (result.kind === 'cancelled' || result.kind === 'indeterminate') return result;
	switch (result.reason) {
		case 'project-environment-conflict':
		case 'project-folder-already-owned':
		case 'project-not-found':
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

function mapEnvironmentConnectionFailure(
	result: DesktopSshProjectEnrollmentResult,
): DesktopSshEnvironmentConnectionResult {
	if (result.kind === 'cancelled') return result;
	if (result.kind === 'indeterminate') {
		return { kind: 'indeterminate', reason: 'control-unavailable' };
	}
	if ('canonicalPath' in result) {
		return { kind: 'failed', reason: 'connection-owner-unavailable' };
	}
	switch (result.reason) {
		case 'busy':
		case 'connection-owner-unavailable':
		case 'environment-not-found':
		case 'local-platform-unsupported':
		case 'registry-unavailable':
		case 'remote-runtime-unavailable':
			return { kind: 'failed', reason: result.reason };
		default:
			return { kind: 'failed', reason: 'connection-owner-unavailable' };
	}
}
