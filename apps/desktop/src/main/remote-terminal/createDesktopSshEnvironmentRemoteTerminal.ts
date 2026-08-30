import { createHash } from 'node:crypto';

import type { DesktopRemoteTerminalCreationRequestId } from '@orchbee/desktop-client-protocol';
import {
	WorkbenchProjectWorkspaceRuntime,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench';
import {
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPresentation,
} from '@orchbee/workbench/document';

import type { DesktopClientSessionCoordinator } from '../client-session';
import {
	EnvironmentIdSchema,
	type DesktopEnvironmentRegistry,
	type EnvironmentId,
} from '../environment';
import type { SshEnvironmentConnectionObservation } from '../ssh-environment';
import type { DesktopSshEnvironmentTerminalRuntimeOwner } from './DesktopSshEnvironmentTerminalRuntimeOwner';
import { resolveDesktopSshEnvironmentRemoteTerminalCreationTarget } from './resolveDesktopSshEnvironmentRemoteTerminalCreationTarget';

interface DesktopSshEnvironmentRemoteTerminalConnectionPort {
	observation(
		environmentId: EnvironmentId,
	):
		| { readonly kind: 'inactive' }
		| { readonly kind: 'ready'; readonly observation: SshEnvironmentConnectionObservation };
}

interface DesktopSshEnvironmentRemoteTerminalOptions {
	readonly allocateIdentifier: () => string;
	readonly connections: DesktopSshEnvironmentRemoteTerminalConnectionPort;
	readonly coordinator: Pick<
		DesktopClientSessionCoordinator,
		'mutateCanonicalDocument' | 'reconcileCanonicalDocument'
	>;
	readonly environmentId: EnvironmentId;
	readonly projectId: string;
	readonly requestId: DesktopRemoteTerminalCreationRequestId;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'get'>;
	readonly runtimeOwner: Pick<DesktopSshEnvironmentTerminalRuntimeOwner, 'prepare'>;
	readonly signal?: AbortSignal | undefined;
	readonly terminalTitle: string;
	readonly workspaceId: string;
}

interface DesktopSshEnvironmentRemoteTerminalIdentity {
	readonly panelId: string;
	readonly presentationId: WorkbenchSharedPresentation['presentationId'];
	readonly tabId: string;
	readonly terminalContextId: string;
}

export type DesktopSshEnvironmentRemoteTerminalResult =
	| (DesktopSshEnvironmentRemoteTerminalIdentity & {
			readonly kind: 'created' | 'existing';
			readonly projectId: string;
			readonly revision: WorkbenchSharedDocument['revision'];
			readonly workspaceId: string;
	  })
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'indeterminate'; readonly reason: 'durability-unconfirmed' }
	| {
			readonly kind: 'failed';
			readonly reason:
				| 'environment-not-connected'
				| 'environment-not-found'
				| 'project-not-found'
				| 'registry-unavailable'
				| 'remote-context-invalid'
				| 'terminal-host-busy'
				| 'terminal-host-compatibility-unsupported'
				| 'terminal-host-unavailable'
				| 'workbench-missing'
				| 'workbench-unavailable'
				| 'workspace-environment-mismatch'
				| 'workspace-not-found';
	  };

export async function createDesktopSshEnvironmentRemoteTerminal(
	options: DesktopSshEnvironmentRemoteTerminalOptions,
): Promise<DesktopSshEnvironmentRemoteTerminalResult> {
	const environmentId = EnvironmentIdSchema.parse(options.environmentId);
	const registered = await options.registry.get(environmentId);
	if (registered.kind === 'unavailable') {
		return { kind: 'failed', reason: 'registry-unavailable' };
	}
	if (registered.value === null) return { kind: 'failed', reason: 'environment-not-found' };
	if (signalIsAborted(options.signal)) return { kind: 'cancelled' };
	if (!environmentIsConnected(options.connections, environmentId)) {
		return { kind: 'failed', reason: 'environment-not-connected' };
	}
	const prepared = await options.runtimeOwner.prepare(environmentId);
	if (prepared.kind === 'failed') {
		return {
			kind: 'failed',
			reason: `terminal-host-${prepared.reason}`,
		};
	}
	if (signalIsAborted(options.signal)) return { kind: 'cancelled' };
	const requestedPresentationId = remoteTerminalPresentationId(options.requestId);

	let terminal:
		| (DesktopSshEnvironmentRemoteTerminalIdentity & {
				readonly kind: 'created' | 'existing';
		  })
		| undefined;
	const mutation = await options.coordinator.mutateCanonicalDocument((canonical) => {
		if (signalIsAborted(options.signal)) {
			return { kind: 'rejected', reason: 'operation-cancelled' };
		}
		if (!environmentIsConnected(options.connections, environmentId)) {
			return { kind: 'rejected', reason: 'environment-not-connected' };
		}
		const target = resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
			canonical,
			options.projectId,
			options.workspaceId,
		);
		if (target.kind === 'rejected') return target;
		if (target.environmentId !== environmentId) {
			return { kind: 'rejected', reason: 'workspace-environment-mismatch' };
		}
		const existing = findRequestedRemoteTerminal(
			target.context,
			target.directoryPath,
			requestedPresentationId,
		);
		if (existing.kind === 'existing') {
			terminal = existing;
			return { kind: 'unchanged' };
		}
		if (existing.kind === 'invalid') {
			return { kind: 'rejected', reason: 'remote-context-invalid' };
		}

		let presentationAllocated = false;
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: canonical,
			standaloneCwd: '/',
			standaloneTerminalTitle: options.terminalTitle,
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`desktop:operation:${options.allocateIdentifier()}`),
				createPresentationId: () => {
					if (presentationAllocated) {
						throw new Error('One remote Terminal request cannot allocate multiple presentations.');
					}
					presentationAllocated = true;
					return requestedPresentationId;
				},
				createRemovalId: () =>
					terminalPresentationRemovalId(`desktop:removal:${options.allocateIdentifier()}`),
				dispatchCommands: () => undefined,
				initialPresentationId: requestedPresentationId,
				kind: 'managed',
			},
		});
		const created = runtime.createEnvironmentTerminalAtLocation({
			environmentId,
			fallbackTitle: options.terminalTitle,
			placement: { groupId: null },
			projectId: options.projectId,
			workspaceId: options.workspaceId,
		});
		if (created.kind === 'rejected') {
			return {
				kind: 'rejected',
				reason: mapWorkbenchRejection(created.reason),
			};
		}
		terminal = { ...created, kind: 'created' };
		return {
			document: runtime.getSharedDocument(
				workbenchSharedDocumentRevision(`${BigInt(canonical.revision) + 1n}`),
			),
			kind: 'commit',
		};
	});

	if (mutation.kind === 'indeterminate') return mutation;
	if (mutation.kind === 'unavailable') {
		return { kind: 'failed', reason: 'workbench-unavailable' };
	}
	if (mutation.kind === 'rejected') {
		if (mutation.reason === 'operation-cancelled') return { kind: 'cancelled' };
		return {
			kind: 'failed',
			reason: mapMutationRejection(mutation.reason),
		};
	}
	if (terminal === undefined) return { kind: 'failed', reason: 'workbench-unavailable' };
	if (mutation.kind === 'unchanged') {
		const reconciled = await options.coordinator.reconcileCanonicalDocument();
		if (reconciled.kind === 'rejected') {
			return {
				kind: 'failed',
				reason:
					reconciled.reason === 'document-missing' ? 'workbench-missing' : 'workbench-unavailable',
			};
		}
	}
	return {
		...terminal,
		projectId: options.projectId,
		revision: mutation.document.revision,
		workspaceId: options.workspaceId,
	};
}

type RequestedRemoteTerminalResult =
	| { readonly kind: 'creatable' }
	| { readonly kind: 'invalid' }
	| (DesktopSshEnvironmentRemoteTerminalIdentity & { readonly kind: 'existing' });

function findRequestedRemoteTerminal(
	context: WorkbenchSharedDocument['terminalContexts'][number],
	directoryPath: string,
	presentationId: WorkbenchSharedPresentation['presentationId'],
): RequestedRemoteTerminalResult {
	if (
		context.panelWorkspace.pendingRemovals.length !== 0 ||
		context.panelWorkspace.tabs.length !== context.tabCollection.tabs.length ||
		context.tabCollection.groups.length !== 0
	) {
		return { kind: 'invalid' };
	}
	let requested: DesktopSshEnvironmentRemoteTerminalIdentity | undefined;
	for (const [index, tab] of context.panelWorkspace.tabs.entries()) {
		const tabRecord = context.tabCollection.tabs[index];
		if (
			tabRecord === undefined ||
			tab.tabId !== tabRecord.id ||
			tabRecord.group !== null ||
			tab.root.kind !== 'panel' ||
			tab.root.childPresentations.length !== 0
		) {
			return { kind: 'invalid' };
		}
		const presentation = tab.root.primaryPresentation;
		if (
			presentation.launchCwd !== directoryPath ||
			(presentation.intent.kind !== 'provision' && presentation.intent.kind !== 'retain')
		) {
			return { kind: 'invalid' };
		}
		if (presentation.presentationId !== presentationId) continue;
		if (requested !== undefined) return { kind: 'invalid' };
		requested = {
			panelId: tab.root.id,
			presentationId: presentation.presentationId,
			tabId: tab.tabId,
			terminalContextId: context.terminalContextId,
		};
	}
	return requested === undefined ? { kind: 'creatable' } : { ...requested, kind: 'existing' };
}

function remoteTerminalPresentationId(
	requestId: DesktopRemoteTerminalCreationRequestId,
): WorkbenchSharedPresentation['presentationId'] {
	const digest = createHash('sha256').update(requestId).digest('hex');
	return terminalPresentationId(`desktop:remote:${digest}`);
}

function environmentIsConnected(
	connections: DesktopSshEnvironmentRemoteTerminalConnectionPort,
	environmentId: EnvironmentId,
): boolean {
	const observation = connections.observation(environmentId);
	return observation.kind === 'ready' && observation.observation.connected;
}

function mapWorkbenchRejection(
	reason: string,
): Extract<DesktopSshEnvironmentRemoteTerminalResult, { readonly kind: 'failed' }>['reason'] {
	switch (reason) {
		case 'project-not-found':
		case 'workspace-not-found':
			return reason;
		case 'workspace-directory-not-remote':
		case 'workspace-environment-mismatch':
			return 'workspace-environment-mismatch';
		default:
			return 'remote-context-invalid';
	}
}

function mapMutationRejection(
	reason: string,
): Extract<DesktopSshEnvironmentRemoteTerminalResult, { readonly kind: 'failed' }>['reason'] {
	switch (reason) {
		case 'document-missing':
			return 'workbench-missing';
		case 'environment-not-connected':
		case 'project-not-found':
		case 'remote-context-invalid':
		case 'workspace-environment-mismatch':
		case 'workspace-not-found':
			return reason;
		default:
			return 'workbench-unavailable';
	}
}

function signalIsAborted(signal: AbortSignal | undefined): boolean {
	return signal?.aborted === true;
}
