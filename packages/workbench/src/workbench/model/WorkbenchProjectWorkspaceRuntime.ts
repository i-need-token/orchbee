import type { TabStripNewTabRequest } from '@orchbee/tab-strip';
import { TabModel } from '@orchbee/tab-strip/model';
import {
	workbenchSharedDocumentRevision,
	type TerminalPresentationEvent,
	type TerminalPresentationFailure,
	type TerminalPresentationId,
	type TerminalPresentationOperationId,
	type TerminalPresentationRemovalId,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
	type WorkbenchSharedPresentation,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
import type { WorkbenchClientView } from '../document/workbench-client-view';
import {
	createProjectWorkspaceState,
	resolveTerminalCwd,
	transitionProjectWorkspace,
	type ProjectWorkspaceEvent,
	type ProjectWorkspaceState,
	type ProjectWorkspaceTransition,
	type ResolveTerminalCwdResult,
	type TerminalContextCommand,
	type WorkbenchResourceScope,
	findWorkbenchWorkspace,
	findWorkspaceResources,
	type WorkbenchWorkspace,
} from '../../project-workspace';
import { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import type { WorkbenchTabTitlePolicy } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import {
	clonePanelWorkspaceState,
	collectPanelLeaves,
	countTerminalsRequiringCloseConfirmation,
	findTerminal,
	type PanelLayoutNode,
	type PanelWorkspaceState,
	type TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalPresentationComposition,
} from './createWorkbenchPreviewTerminalPresentationComposition';
import {
	hydrateWorkbenchDocuments,
	projectWorkbenchClientView,
	projectWorkbenchSharedDocument,
	type HydratedWorkbenchDocumentTerminalContext,
} from './workbench-document-runtime';
import type { WorkbenchPublication } from './workbench-publication';
import { workbenchActiveLocationsEqual } from './workbench-terminal-location-activation';
import type { WorkbenchTerminalMetadataObservation } from './resolveTerminalMetadataObservation';
import {
	revalidateCurrentDirectoryTerminalTarget,
	resolveCurrentDirectoryTerminalTarget,
	type CurrentDirectoryTerminalTarget,
	type CurrentDirectoryTerminalTargetResolution,
} from './resolve-current-directory-terminal-target';
import type {
	WorkbenchLocationRef,
	WorkbenchTerminalCreateIntent,
	WorkbenchTerminalCreateOperationInput,
	WorkbenchTerminalCreateOperationResult,
} from '../semantic-commands/semantic-command-contracts';

type Listener = () => void;

export interface WorkbenchTerminalContext {
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly tabModel: TabModel;
	readonly terminalContextId: string;
}

interface MutableWorkbenchTerminalContext extends WorkbenchTerminalContext {
	nextTabSequence: number;
}

type WorkbenchTerminalTargetResult =
	| {
			readonly context: MutableWorkbenchTerminalContext;
			readonly kind: 'resolved';
			readonly resources: WorkbenchResourceScope | null;
			readonly terminalContextId: string;
			readonly workspace: WorkbenchWorkspace | null;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: 'project-not-found' | 'workspace-not-found';
	  };

interface WorkbenchProjectWorkspaceRuntimeOptions {
	readonly initialClientView?: WorkbenchClientView | undefined;
	readonly initialDocument?: WorkbenchSharedDocument | undefined;
	readonly initialPublication?: WorkbenchPublication | undefined;
	readonly standaloneCwd: string;
	readonly standalonePresentations?: readonly WorkbenchStandaloneTerminalPresentation[] | undefined;
	readonly standaloneTerminalTitle: string;
	readonly terminalPresentation: WorkbenchTerminalPresentationComposition;
}

interface WorkbenchStandaloneTerminalPresentation {
	readonly cwd: string;
	readonly presentationId: TerminalPresentationId;
	readonly title: string;
}

export type WorkbenchTerminalPresentationEventResult =
	| { readonly kind: 'applied' | 'unchanged' }
	| { readonly kind: 'rejected'; readonly reason: string };

type WorkbenchSharedDocumentApplyResult =
	| { readonly kind: 'applied'; readonly clientView: WorkbenchClientView }
	| { readonly kind: 'unchanged' };

type WorkbenchRuntimeTerminalCreateInput = TabStripNewTabRequest & {
	readonly title: string;
} & (
		| {
				readonly currentDirectoryTarget?: never;
				readonly directoryId?: string | undefined;
		  }
		| {
				readonly currentDirectoryTarget: CurrentDirectoryTerminalTarget;
				readonly directoryId?: never;
		  }
	);

export type WorkbenchRuntimeTerminalCreateResult =
	| WorkbenchTerminalCreateOperationResult
	| Extract<ResolveTerminalCwdResult, { kind: 'rejected' }>
	| {
			readonly kind: 'rejected';
			readonly reason:
				| 'active-tab-unavailable'
				| 'panel-unavailable'
				| 'terminal-current-directory-unavailable'
				| 'terminal-target-stale';
	  };

interface WorkbenchActiveTerminalCreationRouteInput {
	readonly directoryId?: string | undefined;
	readonly placement: {
		readonly groupId: string | null;
		readonly index?: number | undefined;
	};
}

type WorkbenchActiveTerminalCreationRoute =
	| { readonly kind: 'local'; readonly location: WorkbenchLocationRef }
	| {
			readonly kind: 'remote';
			readonly workspaceId: string;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason:
				| 'project-not-found'
				| 'remote-terminal-placement-unsupported'
				| 'workspace-directory-not-remote'
				| 'workspace-not-found';
	  };

interface WorkbenchPublishedTerminalIdentity {
	readonly panelId: string;
	readonly presentationId: string;
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly workspaceId: string;
}

interface WorkbenchEnvironmentTerminalCreateInput {
	readonly environmentId: string;
	readonly fallbackTitle: string;
	readonly placement: {
		readonly groupId: string | null;
		readonly index?: number | undefined;
	};
	readonly workspaceId: string;
}

interface WorkbenchTerminalSessionStopFailure {
	readonly failure: TerminalPresentationFailure;
	readonly presentationId: TerminalPresentationId;
}

export interface WorkbenchTerminalSessionStopSnapshot {
	readonly complete: boolean;
	readonly failures: readonly WorkbenchTerminalSessionStopFailure[];
	readonly presentationCount: number;
	readonly stoppedCount: number;
	readonly stoppingCount: number;
}

export type WorkbenchTerminalSessionRestartStageResult =
	| { readonly kind: 'staged'; readonly presentationCount: number }
	| { readonly kind: 'rejected'; readonly reason: string };

export type WorkbenchTerminalSessionStopRequestResult =
	| {
			readonly commandCount: number;
			readonly kind: 'started' | 'unchanged';
			readonly snapshot: WorkbenchTerminalSessionStopSnapshot;
	  }
	| { readonly kind: 'rejected'; readonly reason: string };

export class WorkbenchProjectWorkspaceRuntime {
	private readonly contexts = new Map<string, MutableWorkbenchTerminalContext>();
	private readonly contextDocumentSubscriptions = new Map<string, () => void>();
	private clientViewFingerprint: string | undefined;
	private readonly clientViewListeners = new Set<Listener>();
	private readonly documentListeners = new Set<Listener>();
	private documentFingerprint: string | undefined;
	private durableReconciliationStarted = false;
	private readonly listeners = new Set<Listener>();
	private publishedContextFingerprints: ReadonlyMap<string, string> = new Map();
	private terminalTopologyFingerprint: string | undefined;
	private readonly terminalTopologyListeners = new Set<Listener>();
	private readonly systemCwd: string;
	private readonly terminalPresentation: WorkbenchTerminalPresentationComposition;
	private terminalCreationSealed = false;
	private state = createProjectWorkspaceState();

	public constructor(options: WorkbenchProjectWorkspaceRuntimeOptions) {
		this.systemCwd = options.standaloneCwd;
		this.terminalPresentation = options.terminalPresentation;
		if (options.initialPublication !== undefined) {
			this.adoptPublication(options.initialPublication, options.initialClientView);
			return;
		}
		if (options.initialDocument !== undefined) {
			const documents = hydrateWorkbenchDocuments(
				options.initialDocument,
				options.initialClientView,
			);
			this.state = documents.projectWorkspace;
			for (const terminalContext of documents.terminalContexts) {
				const tabModel = new TabModel(terminalContext.tabCollection);
				this.contexts.set(terminalContext.terminalContextId, {
					nextTabSequence: terminalContext.nextTabSequence,
					panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
						composition: this.terminalPresentation,
						initialState: terminalContext.panelWorkspace,
						initialTabTitlePolicies: terminalContext.tabTitlePolicies,
					}),
					tabModel,
					terminalContextId: terminalContext.terminalContextId,
				});
			}
			return;
		}
		this.contexts.set(
			this.state.standaloneTerminalContextId,
			this.createTerminalContext(
				this.state.standaloneTerminalContextId,
				options.standalonePresentations === undefined ||
					options.standalonePresentations.length === 0
					? [
							{
								cwd: options.standaloneCwd,
								presentationId: options.terminalPresentation.initialPresentationId,
								title: options.standaloneTerminalTitle,
							},
						]
					: options.standalonePresentations,
			),
		);
	}

	public readonly getSnapshot = (): ProjectWorkspaceState => this.state;

	public getSharedDocument(revision: WorkbenchSharedDocumentRevision): WorkbenchSharedDocument {
		return projectWorkbenchSharedDocument(this.runtimeSnapshot(), revision);
	}

	public getClientView(): WorkbenchClientView {
		return projectWorkbenchClientView(this.runtimeSnapshot());
	}

	public applySharedDocument(
		document: WorkbenchSharedDocument,
	): WorkbenchSharedDocumentApplyResult {
		const projected = this.getSharedDocument(document.revision);
		if (sharedDocumentFingerprint(projected) === sharedDocumentFingerprint(document)) {
			return { kind: 'unchanged' };
		}
		const hydrated = hydrateWorkbenchDocuments(document, this.getClientView());
		const previousSharedContexts = new Map(
			projected.terminalContexts.map((context) => [context.terminalContextId, context]),
		);
		const nextSharedContexts = new Map(
			document.terminalContexts.map((context) => [context.terminalContextId, context]),
		);
		const currentTerminals = collectRuntimeTerminals(this.contexts);
		const currentPresentations = collectSharedPresentations(projected);
		const incomingPresentations = collectSharedPresentations(document);
		this.state = {
			...hydrated.projectWorkspace,
			activeLocation: retainActiveLocation(
				this.state.activeLocation,
				hydrated.projectWorkspace.activeLocation,
			),
		};
		this.replaceTerminalContexts(
			hydrated.terminalContexts,
			(terminalContextId) =>
				JSON.stringify(previousSharedContexts.get(terminalContextId)) ===
				JSON.stringify(nextSharedContexts.get(terminalContextId)),
			(terminalContext, currentState) =>
				preserveRuntimePanelWorkspaceState(
					terminalContext.panelWorkspace,
					currentState,
					currentTerminals,
					currentPresentations,
					incomingPresentations,
				),
		);
		this.emit();
		return { clientView: hydrated.clientView, kind: 'applied' };
	}

	/** Takes what an outside authority published as the whole shared state of this runtime. */
	public applyPublishedWorkbench(publication: WorkbenchPublication): void {
		this.adoptPublication(publication, this.getClientView());
		this.emit();
	}

	private adoptPublication(
		publication: WorkbenchPublication,
		candidate: WorkbenchClientView | undefined,
	): void {
		const hydrated = hydrateWorkbenchDocuments(publication.document, candidate);
		const observedTerminals = collectRuntimeTerminals(this.contexts);
		const previousFingerprints = this.publishedContextFingerprints;
		this.publishedContextFingerprints = publication.contextFingerprints;
		this.state = publishedProjectWorkspaceState(this.state, hydrated.projectWorkspace);
		this.replaceTerminalContexts(
			hydrated.terminalContexts,
			(terminalContextId) =>
				previousFingerprints.get(terminalContextId) ===
				publication.contextFingerprints.get(terminalContextId),
			(terminalContext, currentState) =>
				publishedPanelWorkspaceState(
					terminalContext.panelWorkspace,
					currentState,
					observedTerminals,
					publication,
				),
		);
	}

	private replaceTerminalContexts(
		terminalContexts: readonly HydratedWorkbenchDocumentTerminalContext[],
		isUnchanged: (terminalContextId: string) => boolean,
		panelWorkspaceFor: (
			terminalContext: HydratedWorkbenchDocumentTerminalContext,
			currentState: PanelWorkspaceState | undefined,
		) => PanelWorkspaceState,
	): void {
		const previousContexts = new Map(this.contexts);
		for (const unsubscribe of this.contextDocumentSubscriptions.values()) unsubscribe();
		this.contextDocumentSubscriptions.clear();
		this.contexts.clear();
		for (const terminalContext of terminalContexts) {
			const existing = previousContexts.get(terminalContext.terminalContextId);
			if (existing !== undefined && isUnchanged(terminalContext.terminalContextId)) {
				this.contexts.set(terminalContext.terminalContextId, existing);
				continue;
			}
			const tabModel = new TabModel(terminalContext.tabCollection);
			this.contexts.set(terminalContext.terminalContextId, {
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
					composition: this.terminalPresentation,
					initialState: panelWorkspaceFor(
						terminalContext,
						existing?.panelWorkspaceStore.getSnapshot(),
					),
					initialTabTitlePolicies: terminalContext.tabTitlePolicies,
				}),
				tabModel,
				terminalContextId: terminalContext.terminalContextId,
			});
		}
	}

	public readonly subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public readonly subscribeClientView = (listener: Listener): (() => void) => {
		if (this.clientViewListeners.size === 0) {
			this.clientViewFingerprint = JSON.stringify(this.getClientView());
		}
		this.clientViewListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.clientViewListeners.delete(listener);
			if (this.clientViewListeners.size === 0) this.clientViewFingerprint = undefined;
			this.syncContextDocumentSubscriptions();
		};
	};

	/** Observe navigation without treating grouping or newly published contexts as navigation. */
	public readonly subscribeNavigation = (listener: Listener): (() => void) => {
		const identity = () => {
			const context = this.getActiveTerminalContext();
			const activeTabId = context.tabModel.getSnapshot().activeTabId;
			const tab = context.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((candidate) => candidate.tabId === activeTabId);
			const view = this.getClientView().terminalContexts.find(
				(candidate) => candidate.terminalContextId === context.terminalContextId,
			);
			return JSON.stringify({
				location: this.state.activeLocation,
				activeTabId,
				panel: tab?.focusedPanelId,
				panels: view?.panelWorkspace.tabs.find((candidate) => candidate.tabId === activeTabId)
					?.panels,
			});
		};
		let previous = identity();
		return this.subscribeClientView(() => {
			const next = identity();
			if (next === previous) return;
			previous = next;
			listener();
		});
	};

	public readonly subscribeDocument = (listener: Listener): (() => void) => {
		if (this.documentListeners.size === 0) {
			this.documentFingerprint = sharedDocumentFingerprint(
				this.getSharedDocument(workbenchSharedDocumentRevision('0')),
			);
		}
		this.documentListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.documentListeners.delete(listener);
			if (this.documentListeners.size === 0) this.documentFingerprint = undefined;
			this.syncContextDocumentSubscriptions();
		};
	};

	public readonly getTerminalTopologyRevision = (): string =>
		JSON.stringify(this.runtimeSnapshot());

	public readonly subscribeTerminalTopology = (listener: Listener): (() => void) => {
		if (this.terminalTopologyListeners.size === 0) {
			this.terminalTopologyFingerprint = this.getTerminalTopologyRevision();
		}
		this.terminalTopologyListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.terminalTopologyListeners.delete(listener);
			if (this.terminalTopologyListeners.size === 0) {
				this.terminalTopologyFingerprint = undefined;
			}
			this.syncContextDocumentSubscriptions();
		};
	};

	public reconcileDurableTerminalPresentations(): {
		readonly commandCount: number;
		readonly kind: 'started' | 'unchanged';
	} {
		if (this.terminalPresentation.kind !== 'managed' || this.durableReconciliationStarted) {
			return { commandCount: 0, kind: 'unchanged' };
		}
		this.durableReconciliationStarted = true;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		let commandCount = 0;
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				for (const panel of collectPanelLeaves(tab.root)) {
					for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
						switch (terminal.lifecycle.kind) {
							case 'provisioning':
								commands.push({
									launchCwd: terminal.launchCwd,
									operationId: terminal.lifecycle.operationId,
									presentationId: terminal.presentationId,
									type: 'terminal-provision-requested',
								});
								commandCount += 1;
								break;
							case 'restoring': {
								const transition = context.panelWorkspaceStore.transition({
									operationId: this.terminalPresentation.createOperationId(),
									presentationId: terminal.presentationId,
									tabId: tab.tabId,
									type: 'terminal-restore-requested',
								});
								if (transition.outcome !== 'applied') {
									throw new Error('A durable terminal restore could not be started.');
								}
								commandCount += transition.commands.length;
								break;
							}
							case 'closing':
								commands.push({
									disposition: 'remove',
									operationId: terminal.lifecycle.operationId,
									presentationId: terminal.presentationId,
									type: 'terminal-close-requested',
								});
								commandCount += 1;
								break;
							case 'stopping':
								commands.push({
									disposition: 'stop',
									operationId: terminal.lifecycle.operationId,
									presentationId: terminal.presentationId,
									type: 'terminal-close-requested',
								});
								commandCount += 1;
								break;
							default:
								break;
						}
					}
				}
			}
		}
		if (commands.length > 0) this.terminalPresentation.dispatchCommands(commands);
		return { commandCount, kind: 'started' };
	}

	public getActiveTerminalContext(): WorkbenchTerminalContext {
		const terminalContextId =
			this.state.activeLocation.kind === 'standalone'
				? this.state.standaloneTerminalContextId
				: this.requireActiveWorkspaceTerminalContextId();
		return this.requireContext(terminalContextId);
	}

	public getTerminalContext(terminalContextId: string): WorkbenchTerminalContext | null {
		return this.contexts.get(terminalContextId) ?? null;
	}

	public hasPublishedTerminal(identity: WorkbenchPublishedTerminalIdentity): boolean {
		const target = this.resolveTerminalTarget({
			kind: 'workspace',

			workspaceId: identity.workspaceId,
		});
		if (target.kind === 'rejected' || target.terminalContextId !== identity.terminalContextId) {
			return false;
		}
		if (!target.context.tabModel.getSnapshot().tabs.some((tab) => tab.id === identity.tabId)) {
			return false;
		}
		const tab = target.context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === identity.tabId);
		if (tab === undefined) return false;
		const panel = collectPanelLeaves(tab.root).find(
			(candidate) => candidate.id === identity.panelId,
		);
		if (panel === undefined) return false;
		return [panel.primaryPresentation, ...panel.childPresentations].some(
			(presentation) => presentation.presentationId === identity.presentationId,
		);
	}

	public getTerminalSessionStopSnapshot(): WorkbenchTerminalSessionStopSnapshot {
		let presentationCount = 0;
		let stoppedCount = 0;
		let stoppingCount = 0;
		const failures: WorkbenchTerminalSessionStopFailure[] = [];
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				for (const panel of collectPanelLeaves(tab.root)) {
					for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
						presentationCount += 1;
						switch (terminal.lifecycle.kind) {
							case 'stopped':
							case 'closed':
								stoppedCount += 1;
								break;
							case 'stopping':
							case 'closing':
								stoppingCount += 1;
								break;
							case 'stop-failed':
							case 'close-failed':
								failures.push({
									failure: terminal.lifecycle.failure,
									presentationId: terminal.presentationId,
								});
								break;
							default:
								break;
						}
					}
				}
			}
		}
		return {
			complete: failures.length === 0 && stoppedCount === presentationCount,
			failures,
			presentationCount,
			stoppedCount,
			stoppingCount,
		};
	}

	public getTerminalSessionCloseConfirmationCount(): number {
		let count = 0;
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				count += countTerminalsRequiringCloseConfirmation(tab.root);
			}
		}
		return count;
	}

	public getRestartableTerminalPresentationIds(): readonly TerminalPresentationId[] {
		return [...this.contexts.values()].flatMap((context) =>
			context.panelWorkspaceStore.getRestartableTerminalPresentationIds(),
		);
	}

	public requestAllTerminalSessionsStop(): WorkbenchTerminalSessionStopRequestResult {
		if (this.terminalPresentation.kind !== 'managed') {
			return { kind: 'rejected', reason: 'managed-terminal-runtime-required' };
		}
		if (this.terminalCreationSealed) {
			return {
				commandCount: 0,
				kind: 'unchanged',
				snapshot: this.getTerminalSessionStopSnapshot(),
			};
		}
		this.terminalCreationSealed = true;
		let commandCount = 0;
		for (const context of this.contexts.values()) {
			context.panelWorkspaceStore.sealTerminalCreation();
			const transition = context.panelWorkspaceStore.requestAllTerminalSessionsStop();
			if (transition.outcome === 'rejected') {
				return { kind: 'rejected', reason: transition.rejection ?? 'terminal-stop-rejected' };
			}
			commandCount += transition.commands.length;
		}
		return {
			commandCount,
			kind: commandCount === 0 ? 'unchanged' : 'started',
			snapshot: this.getTerminalSessionStopSnapshot(),
		};
	}

	public retryFailedTerminalSessionStops(): WorkbenchTerminalSessionStopRequestResult {
		if (!this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-stop-not-started' };
		}
		let commandCount = 0;
		for (const context of this.contexts.values()) {
			const transition = context.panelWorkspaceStore.retryFailedTerminalSessionStops();
			if (transition.outcome === 'rejected') {
				return {
					kind: 'rejected',
					reason: transition.rejection ?? 'terminal-stop-retry-rejected',
				};
			}
			commandCount += transition.commands.length;
		}
		return {
			commandCount,
			kind: commandCount === 0 ? 'unchanged' : 'started',
			snapshot: this.getTerminalSessionStopSnapshot(),
		};
	}

	public stageStoppedTerminalSessionsForRestart(
		presentationIds: readonly TerminalPresentationId[],
	): WorkbenchTerminalSessionRestartStageResult {
		const composition = this.terminalPresentation;
		if (composition.kind !== 'managed') {
			return { kind: 'rejected', reason: 'managed-terminal-runtime-required' };
		}
		if (!this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-stop-not-started' };
		}
		if (!this.getTerminalSessionStopSnapshot().complete) {
			return { kind: 'rejected', reason: 'terminal-stop-incomplete' };
		}
		const requestedIds = new Set(presentationIds);
		if (requestedIds.size !== presentationIds.length) {
			return { kind: 'rejected', reason: 'terminal-presentation-conflict' };
		}
		const targets: Array<{
			readonly context: MutableWorkbenchTerminalContext;
			readonly presentationId: TerminalPresentationId;
		}> = [];
		for (const presentationId of presentationIds) {
			const matches = [...this.contexts.values()].filter((context) =>
				context.panelWorkspaceStore
					.getSnapshot()
					.tabs.some((tab) => findTerminal(tab.root, presentationId) !== null),
			);
			const context = matches.length === 1 ? matches[0] : undefined;
			if (context === undefined) {
				return { kind: 'rejected', reason: 'terminal-session-not-stopped' };
			}
			const terminal = context.panelWorkspaceStore
				.getSnapshot()
				.tabs.flatMap((tab) => collectPanelLeaves(tab.root))
				.flatMap((panel) => [panel.primaryPresentation, ...panel.childPresentations])
				.find((candidate) => candidate.presentationId === presentationId);
			if (terminal?.lifecycle.kind !== 'stopped') {
				return { kind: 'rejected', reason: 'terminal-session-not-stopped' };
			}
			targets.push({ context, presentationId });
		}
		const currentIds = new Set(
			[...this.contexts.values()].flatMap((context) =>
				context.panelWorkspaceStore
					.getSnapshot()
					.tabs.flatMap((tab) => collectPanelLeaves(tab.root))
					.flatMap((panel) => [
						panel.primaryPresentation.presentationId,
						...panel.childPresentations.map((terminal) => terminal.presentationId),
					]),
			),
		);
		const replacements = targets.map((target) => ({
			context: target.context,
			oldPresentationId: target.presentationId,
			presentation: {
				kind: 'managed' as const,
				operationId: composition.createOperationId(),
				presentationId: composition.createPresentationId(),
			},
		}));
		const replacementIds = replacements.map((item) => item.presentation.presentationId);
		if (
			new Set(replacementIds).size !== replacementIds.length ||
			replacementIds.some((presentationId) => currentIds.has(presentationId))
		) {
			return { kind: 'rejected', reason: 'terminal-presentation-conflict' };
		}
		for (const replacement of replacements) {
			const transition =
				replacement.context.panelWorkspaceStore.stageStoppedTerminalSessionForRestart(
					replacement.oldPresentationId,
					replacement.presentation,
				);
			if (transition.outcome === 'rejected') {
				throw new Error('A validated terminal restart transition was rejected.');
			}
		}
		return { kind: 'staged', presentationCount: replacements.length };
	}

	public startStoppedTerminalSession(
		presentationId: TerminalPresentationId,
	): WorkbenchTerminalPresentationEventResult {
		if (this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-creation-sealed' };
		}
		const matches = [...this.contexts.values()].filter((context) =>
			context.panelWorkspaceStore
				.getSnapshot()
				.tabs.some((tab) => findTerminal(tab.root, presentationId) !== null),
		);
		const match = matches.length === 1 ? matches[0] : undefined;
		if (match === undefined) {
			return {
				kind: 'rejected',
				reason: matches.length === 0 ? 'terminal-not-found' : 'terminal-presentation-conflict',
			};
		}
		const composition = this.terminalPresentation;
		if (composition.kind !== 'managed') {
			return { kind: 'rejected', reason: 'managed-terminal-runtime-required' };
		}
		const nextPresentationId = composition.createPresentationId();
		if (
			[...this.contexts.values()].some((context) =>
				context.panelWorkspaceStore
					.getSnapshot()
					.tabs.some((tab) => findTerminal(tab.root, nextPresentationId) !== null),
			)
		) {
			return { kind: 'rejected', reason: 'terminal-presentation-conflict' };
		}
		const transition = match.panelWorkspaceStore.startStoppedTerminalSession(presentationId, {
			kind: 'managed',
			operationId: composition.createOperationId(),
			presentationId: nextPresentationId,
		});
		return transition.outcome === 'rejected'
			? { kind: 'rejected', reason: transition.rejection ?? 'terminal-start-rejected' }
			: { kind: transition.outcome };
	}

	public getActiveDefaultCwd(): string {
		const result = this.resolveActiveTerminalCwd();
		if (result.kind === 'rejected') {
			throw new Error('A default CWD unexpectedly used an explicit Directory.');
		}
		return result.cwd;
	}

	public resolveActiveTerminalCurrentDirectoryTarget(
		targetPanelId?: string,
	): CurrentDirectoryTerminalTargetResolution {
		const context = this.getActiveMutableContext();
		return resolveCurrentDirectoryTerminalTarget({
			activeTabId: context.tabModel.getSnapshot().activeTabId,
			state: context.panelWorkspaceStore.getSnapshot(),
			targetPanelId: targetPanelId ?? null,
			terminalContextId: context.terminalContextId,
		});
	}

	public resolveTerminalCurrentDirectoryTarget(
		target: CurrentDirectoryTerminalTarget,
	): CurrentDirectoryTerminalTargetResolution {
		const context = this.getActiveMutableContext();
		return revalidateCurrentDirectoryTerminalTarget({
			activeTabId: context.tabModel.getSnapshot().activeTabId,
			state: context.panelWorkspaceStore.getSnapshot(),
			target,
			terminalContextId: context.terminalContextId,
		});
	}

	public createDefaultWorkspace(
		name: string,
		terminalTitle: string,
		beforeCommit?: () => void,
	): ProjectWorkspaceTransition {
		const names = new Set(this.state.workspaces.map((workspace) => workspace.name));
		let availableName = name;
		for (let sequence = 2; names.has(availableName); sequence += 1)
			availableName = `${name} (${sequence})`;
		return this.transition(
			{
				type: 'workspace-created-default',
				name: availableName,
				terminalTitle,
				directory: {
					id: 'default-directory',
					name,
					path: this.systemCwd,
					available: true,
					git: null,
					kind: 'folder',
					placement: { kind: 'local' },
					source: 'selected',
				},
			},
			beforeCommit,
		);
	}

	public resolveActiveTerminalCwd(directoryId?: string): ResolveTerminalCwdResult {
		if (this.state.activeLocation.kind === 'standalone') {
			return resolveTerminalCwd({
				directories: [],
				...(directoryId === undefined ? {} : { explicitDirectoryId: directoryId }),
				systemCwd: this.systemCwd,
			});
		}
		const activeLocation = this.state.activeLocation;
		const workspace = findWorkbenchWorkspace(this.state, activeLocation.workspaceId);
		const project = findWorkspaceResources(this.state, activeLocation.workspaceId);
		if (project === undefined || workspace === undefined) {
			throw new Error('The active Workspace default CWD cannot be resolved.');
		}
		const result = resolveTerminalCwd({
			directories: project.directories,
			...(directoryId === undefined ? {} : { explicitDirectoryId: directoryId }),
			projectPrimaryDirectoryId: project.primaryDirectoryId,
			systemCwd: this.systemCwd,
			workspacePrimaryDirectoryId: workspace.primaryDirectoryId,
		});
		return result;
	}

	public getActiveNextTerminalSequence(): number {
		return this.getActiveMutableContext().nextTabSequence;
	}

	public requestActiveTerminalTabsClose(
		tabIds: readonly string[],
	): WorkbenchTerminalPresentationEventResult {
		const transition = this.getActiveMutableContext().panelWorkspaceStore.requestTabClose(tabIds);
		return transition.outcome === 'rejected'
			? { kind: 'rejected', reason: transition.rejection ?? 'terminal-close-rejected' }
			: { kind: transition.outcome };
	}

	public getActiveTerminalTabsCloseConfirmationCount(tabIds: readonly string[]): number {
		const requestedTabIds = new Set(tabIds);
		return this.getActiveMutableContext()
			.panelWorkspaceStore.getSnapshot()
			.tabs.reduce(
				(count, tab) =>
					requestedTabIds.has(tab.tabId)
						? count + countTerminalsRequiringCloseConfirmation(tab.root)
						: count,
				0,
			);
	}

	/** How many terminals these Tabs hold, whatever each one is doing. */
	public getActiveTerminalTabsTerminalCount(tabIds: readonly string[]): number {
		const requestedTabIds = new Set(tabIds);
		return this.getActiveMutableContext()
			.panelWorkspaceStore.getSnapshot()
			.tabs.reduce(
				(count, tab) =>
					requestedTabIds.has(tab.tabId)
						? collectPanelLeaves(tab.root).reduce(
								(terminals, panel) => terminals + 1 + panel.childPresentations.length,
								count,
							)
						: count,
				0,
			);
	}

	public applyTerminalPresentationEvent(
		event: TerminalPresentationEvent,
	): WorkbenchTerminalPresentationEventResult {
		const matches: Array<{
			readonly store: PanelWorkspaceStore;
			readonly tabId: string;
			readonly terminal: TerminalPresentation;
		}> = [];
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				const terminal = findTerminal(tab.root, event.presentationId);
				if (terminal !== null) {
					matches.push({ store: context.panelWorkspaceStore, tabId: tab.tabId, terminal });
				}
			}
		}
		const match = matches.length === 1 ? matches[0] : undefined;
		if (match === undefined) {
			return {
				kind: 'rejected',
				reason: matches.length === 0 ? 'terminal-not-found' : 'terminal-presentation-conflict',
			};
		}
		let transition: ReturnType<PanelWorkspaceStore['transition']>;
		if (event.type === 'terminal-close-completed' || event.type === 'terminal-close-failed') {
			if (
				match.terminal.lifecycle.kind === 'stopping' &&
				match.terminal.lifecycle.operationId === event.operationId
			) {
				transition = match.store.transition(
					event.type === 'terminal-close-completed'
						? { ...event, tabId: match.tabId, type: 'terminal-stop-completed' }
						: { ...event, tabId: match.tabId, type: 'terminal-stop-failed' },
				);
			} else {
				const removalId = removalIdForEvent(match.terminal, event);
				if (removalId === null) return { kind: 'rejected', reason: 'stale-operation' };
				transition = match.store.transition({
					...event,
					removalId: requireRemovalId(removalId),
					tabId: match.tabId,
				});
			}
		} else if (event.type === 'terminal-exited') {
			transition = match.store.applyTerminalExit({ ...event, tabId: match.tabId });
		} else {
			transition = match.store.transition({ ...event, tabId: match.tabId });
		}
		return transition.outcome === 'rejected'
			? { kind: 'rejected', reason: transition.rejection ?? 'terminal-event-rejected' }
			: { kind: transition.outcome };
	}

	public applyTerminalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: WorkbenchTerminalMetadataObservation,
	): WorkbenchTerminalPresentationEventResult {
		return this.#applyTerminalMetadataObservation(
			presentationId,
			observation,
			observation.closeActivity,
		);
	}

	public applyTerminalNaturalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: Omit<WorkbenchTerminalMetadataObservation, 'closeActivity'>,
	): WorkbenchTerminalPresentationEventResult {
		return this.#applyTerminalMetadataObservation(presentationId, observation);
	}

	#applyTerminalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: Omit<WorkbenchTerminalMetadataObservation, 'closeActivity'>,
		closeActivity?: WorkbenchTerminalMetadataObservation['closeActivity'],
	): WorkbenchTerminalPresentationEventResult {
		const matches: Array<{
			readonly store: PanelWorkspaceStore;
			readonly tabId: string;
			readonly terminal: TerminalPresentation;
		}> = [];
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				const terminal = findTerminal(tab.root, presentationId);
				if (terminal !== null) {
					matches.push({ store: context.panelWorkspaceStore, tabId: tab.tabId, terminal });
				}
			}
		}
		const match = matches.length === 1 ? matches[0] : undefined;
		if (match === undefined) {
			return {
				kind: 'rejected',
				reason: matches.length === 0 ? 'terminal-not-found' : 'terminal-presentation-conflict',
			};
		}
		const transition = match.store.transition({
			...(observation.currentDirectory.kind === 'reported'
				? { currentCwd: observation.currentDirectory.currentCwd }
				: {}),
			closeActivity: closeActivity ?? match.terminal.closeActivity,
			naturalTitle: observation.naturalTitle,
			presentationId,
			tabId: match.tabId,
			type: 'terminal-metadata-changed',
		});
		return transition.outcome === 'rejected'
			? { kind: 'rejected', reason: transition.rejection ?? 'terminal-event-rejected' }
			: { kind: transition.outcome };
	}

	public transition(
		event: ProjectWorkspaceEvent,
		beforeCommit?: (() => void) | undefined,
	): ProjectWorkspaceTransition & {
		readonly projectCreationReceipt?: WorkbenchProjectCreationReceipt;
	} {
		const result = transitionProjectWorkspace(this.state, event);
		if (result.outcome !== 'applied') return result;
		if (
			this.terminalCreationSealed &&
			result.commands.some(
				(command) =>
					command.type === 'terminal-context-created' ||
					command.type === 'terminal-context-created-empty' ||
					command.type === 'terminal-context-cloned',
			)
		) {
			throw new Error('Terminal context creation is sealed for application shutdown.');
		}
		const preparedContexts = new Map(this.contexts);
		for (const command of result.commands) this.prepareContextCommand(preparedContexts, command);
		beforeCommit?.();
		this.contexts.clear();
		for (const [id, context] of preparedContexts) this.contexts.set(id, context);
		this.state = result.state;
		// Capture creation identity before observers can publish or replace this context.
		const location = result.state.activeLocation;
		const workspace =
			location.kind === 'workspace'
				? findWorkbenchWorkspace(result.state, location.workspaceId)
				: undefined;
		const panel =
			workspace === undefined
				? undefined
				: this.contexts.get(workspace.terminalContextId)?.panelWorkspaceStore.getSnapshot().tabs[0]
						?.root;
		const projectCreationReceipt =
			event.type === 'project-created' && workspace !== undefined && panel?.kind === 'panel'
				? {
						directoryId: event.directory.id,
						path: event.directory.path,
						presentationId: panel.primaryPresentation.presentationId,
						projectId: workspace.projectId,
						resourceScopeId: workspace.resourceScopeId,
						terminalContextId: workspace.terminalContextId,
						workspaceId: workspace.id,
					}
				: undefined;
		this.emit();
		return projectCreationReceipt === undefined ? result : { ...result, projectCreationReceipt };
	}

	public createTerminal(
		input: WorkbenchRuntimeTerminalCreateInput,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchRuntimeTerminalCreateResult {
		const activeLocation = this.state.activeLocation;
		const location: WorkbenchLocationRef =
			activeLocation.kind === 'standalone'
				? { kind: 'standalone' }
				: {
						kind: 'workspace',

						workspaceId: activeLocation.workspaceId,
					};
		const placement = {
			groupId: input.groupId,
			...(input.index === undefined ? {} : { index: input.index }),
		};
		let result: WorkbenchRuntimeTerminalCreateResult;
		if (input.currentDirectoryTarget === undefined) {
			result = this.createTerminalAtLocation(
				{
					...(input.directoryId === undefined ? {} : { directoryId: input.directoryId }),
					fallbackTitle: input.title,
					location,
					placement,
				},
				beforeCommit,
			);
		} else {
			const cwdResult = this.resolveTerminalCurrentDirectoryTarget(input.currentDirectoryTarget);
			if (cwdResult.kind === 'unavailable') {
				return { kind: 'rejected', reason: cwdResult.reason };
			}
			const target = this.resolveTerminalTarget(location);
			if (target.kind === 'rejected') return target;
			result = this.createTerminalInTarget(
				target,
				cwdResult.cwd,
				{
					fallbackTitle: input.title,
					placement,
				},
				beforeCommit,
			);
		}
		if (result.kind === 'created') {
			this.requireContext(result.terminalContextId).tabModel.selectTab(result.tabId);
		}
		return result;
	}

	public resolveActiveTerminalCreationRoute(
		input: WorkbenchActiveTerminalCreationRouteInput,
	): WorkbenchActiveTerminalCreationRoute {
		const activeLocation = this.state.activeLocation;
		if (activeLocation.kind === 'standalone') {
			return { kind: 'local', location: { kind: 'standalone' } };
		}
		const target = this.resolveTerminalTarget({
			kind: 'workspace',

			workspaceId: activeLocation.workspaceId,
		});
		if (target.kind === 'rejected') return target;
		const primaryDirectory = target.resources?.directories.find(
			(candidate) => candidate.id === target.workspace?.primaryDirectoryId,
		);
		if (primaryDirectory?.placement.kind !== 'environment') {
			return {
				kind: 'local',
				location: {
					kind: 'workspace',

					workspaceId: activeLocation.workspaceId,
				},
			};
		}
		if (
			input.directoryId !== undefined ||
			input.placement.groupId !== null ||
			input.placement.index !== undefined
		) {
			return { kind: 'rejected', reason: 'remote-terminal-placement-unsupported' };
		}
		return {
			kind: 'remote',

			workspaceId: activeLocation.workspaceId,
		};
	}

	public createTerminalAtLocation(
		input: WorkbenchTerminalCreateOperationInput,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchTerminalCreateOperationResult {
		if (this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-creation-sealed' };
		}
		const target = this.resolveTerminalTarget(input.location);
		if (target.kind === 'rejected') return target;
		const cwdResult = this.resolveTerminalCwdForTarget(target, input.directoryId);
		if (cwdResult.kind === 'rejected') return cwdResult;
		return this.createTerminalInTarget(target, cwdResult.cwd, input, beforeCommit);
	}

	public createEnvironmentTerminalAtLocation(
		input: WorkbenchEnvironmentTerminalCreateInput,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchTerminalCreateOperationResult {
		if (this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-creation-sealed' };
		}
		const target = this.resolveTerminalTarget({
			kind: 'workspace',

			workspaceId: input.workspaceId,
		});
		if (target.kind === 'rejected') return target;
		const directory = target.resources?.directories.find(
			(candidate) => candidate.id === target.workspace?.primaryDirectoryId,
		);
		if (directory?.placement.kind !== 'environment') {
			return { kind: 'rejected', reason: 'workspace-directory-not-remote' };
		}
		if (directory.placement.environmentId !== input.environmentId) {
			return { kind: 'rejected', reason: 'workspace-environment-mismatch' };
		}
		return this.createTerminalInTarget(target, directory.path, input, beforeCommit);
	}

	private createTerminalInTarget(
		target: Extract<WorkbenchTerminalTargetResult, { readonly kind: 'resolved' }>,
		cwd: string,
		input: Pick<WorkbenchTerminalCreateOperationInput, 'fallbackTitle' | 'placement'>,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchTerminalCreateOperationResult {
		const context = target.context;
		const tabId = `${target.terminalContextId}-tab-${context.nextTabSequence}`;
		const activateCreatedTab = context.tabModel.getSnapshot().tabs.length === 0;
		if (!canInsertTerminalTab(context.tabModel, tabId, input, activateCreatedTab)) {
			return { kind: 'rejected', reason: 'tab-placement-invalid' };
		}
		const presentation = context.panelWorkspaceStore.createTerminalPresentation();
		if (collectRuntimeTerminals(this.contexts).has(presentation.presentationId)) {
			return { kind: 'rejected', reason: 'terminal-presentation-conflict' };
		}
		if (presentation.kind === 'managed' && this.hasTerminalOperationId(presentation.operationId)) {
			return { kind: 'rejected', reason: 'terminal-operation-conflict' };
		}
		beforeCommit?.();
		context.nextTabSequence += 1;
		context.panelWorkspaceStore.prepareTabCwd(tabId, cwd, presentation);
		context.tabModel.addTab(
			{
				group: input.placement.groupId,
				pinned: false,
				title: input.fallbackTitle,
			},
			{
				activate: activateCreatedTab,
				id: tabId,
				...(input.placement.index === undefined ? {} : { index: input.placement.index }),
			},
		);
		context.panelWorkspaceStore.reconcileTabs();
		const createdTab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === tabId);
		if (createdTab?.root.kind !== 'panel') {
			throw new Error('A newly created top-level Terminal Tab must own one primary Panel.');
		}
		const terminal = createdTab.root.primaryPresentation;
		return {
			intent: createdTerminalIntent(terminal.lifecycle),
			kind: 'created',
			panelId: createdTab.root.id,
			presentationId: terminal.presentationId,
			tabId,
			terminalContextId: target.terminalContextId,
		};
	}

	private resolveTerminalTarget(location: WorkbenchLocationRef): WorkbenchTerminalTargetResult {
		if (location.kind === 'standalone') {
			return {
				context: this.requireContext(this.state.standaloneTerminalContextId),
				kind: 'resolved',
				resources: null,
				terminalContextId: this.state.standaloneTerminalContextId,
				workspace: null,
			};
		}
		const workspace = findWorkbenchWorkspace(this.state, location.workspaceId);
		if (workspace === undefined) return { kind: 'rejected', reason: 'workspace-not-found' };
		const resources = findWorkspaceResources(this.state, workspace.id);
		if (resources === undefined) throw new Error('A Workspace has lost its launch resources.');
		return {
			context: this.requireContext(workspace.terminalContextId),
			kind: 'resolved',
			resources,
			terminalContextId: workspace.terminalContextId,
			workspace,
		};
	}

	private resolveTerminalCwdForTarget(
		target: Extract<WorkbenchTerminalTargetResult, { readonly kind: 'resolved' }>,
		directoryId?: string | undefined,
	): ResolveTerminalCwdResult {
		return resolveTerminalCwd({
			directories: target.resources?.directories ?? [],
			...(directoryId === undefined ? {} : { explicitDirectoryId: directoryId }),
			...(target.resources === null
				? {}
				: { projectPrimaryDirectoryId: target.resources.primaryDirectoryId }),
			systemCwd: this.systemCwd,
			...(target.workspace === null
				? {}
				: { workspacePrimaryDirectoryId: target.workspace.primaryDirectoryId }),
		});
	}

	private hasTerminalOperationId(operationId: TerminalPresentationOperationId): boolean {
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				for (const panel of collectPanelLeaves(tab.root)) {
					for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
						if (terminalLifecycleOperationId(terminal.lifecycle) === operationId) return true;
					}
				}
			}
		}
		return false;
	}

	private prepareContextCommand(
		contexts: Map<string, MutableWorkbenchTerminalContext>,
		command: TerminalContextCommand,
	): void {
		switch (command.type) {
			case 'terminal-context-created-empty':
				contexts.set(
					command.terminalContextId,
					this.createEmptyTerminalContext(command.terminalContextId),
				);
				break;
			case 'terminal-context-created':
				contexts.set(
					command.terminalContextId,
					this.createTerminalContext(command.terminalContextId, [
						{
							cwd: command.cwd,
							presentationId: this.terminalPresentation.createPresentationId(),
							title: command.terminalTitle,
						},
					]),
				);
				break;
			case 'terminal-context-cloned':
				contexts.set(
					command.targetTerminalContextId,
					this.cloneTerminalContext(
						command.sourceTerminalContextId,
						command.targetTerminalContextId,
					),
				);
				break;
			case 'terminal-context-disposed':
				contexts.delete(command.terminalContextId);
				break;
		}
	}

	private createEmptyTerminalContext(terminalContextId: string): MutableWorkbenchTerminalContext {
		const tabModel = new TabModel({ activeTabId: null, tabs: [] });
		return {
			nextTabSequence: 1,
			panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
				composition: this.terminalPresentation,
				initialState: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
			}),
			tabModel,
			terminalContextId,
		};
	}

	private createTerminalContext(
		terminalContextId: string,
		presentations: readonly WorkbenchStandaloneTerminalPresentation[],
	): MutableWorkbenchTerminalContext {
		if (presentations.length === 0)
			throw new Error('A terminal context requires one presentation.');
		const tabModel = new TabModel({
			activeTabId: `${terminalContextId}-tab-1`,
			tabs: presentations.map((presentation, index) => ({
				group: null,
				id: `${terminalContextId}-tab-${index + 1}`,
				pinned: false,
				title: presentation.title,
			})),
		});
		return {
			nextTabSequence: presentations.length + 1,
			panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
				composition: this.terminalPresentation,
				initialPresentations: presentations.map((presentation) => ({
					cwd: presentation.cwd,
					presentationId: presentation.presentationId,
				})),
			}),
			tabModel,
			terminalContextId,
		};
	}

	private cloneTerminalContext(
		sourceTerminalContextId: string,
		targetTerminalContextId: string,
	): MutableWorkbenchTerminalContext {
		if (this.terminalPresentation.kind === 'managed') {
			throw new Error('Managed terminal composition does not support cloned terminal contexts.');
		}
		const source = this.requireContext(sourceTerminalContextId);
		source.panelWorkspaceStore.reconcileTabs();
		const sourceSnapshot = source.tabModel.getSnapshot();
		const tabIdMap = new Map<string, string>();
		const groupIdMap = new Map<string, string>();
		for (const [index, group] of sourceSnapshot.groups.entries()) {
			groupIdMap.set(group.id, `${targetTerminalContextId}-group-${index + 1}`);
		}
		for (const [index, tab] of sourceSnapshot.tabs.entries()) {
			tabIdMap.set(tab.id, `${targetTerminalContextId}-tab-${index + 1}`);
		}
		const tabModel = new TabModel({
			activeTabId:
				sourceSnapshot.activeTabId === null
					? null
					: (tabIdMap.get(sourceSnapshot.activeTabId) ?? null),
			groups: sourceSnapshot.groups.map((group) => ({
				collapsed: group.collapsed,
				color: group.color,
				id: requireMappedId(groupIdMap, group.id, 'Tab group'),
				title: group.title,
			})),
			mostRecentGroupId:
				sourceSnapshot.mostRecentGroupId === null
					? null
					: requireMappedId(groupIdMap, sourceSnapshot.mostRecentGroupId, 'recent Tab group'),
			selectedTabIds: [...sourceSnapshot.selectedTabIds].map((tabId) =>
				requireMappedId(tabIdMap, tabId, 'selected Tab'),
			),
			tabs: sourceSnapshot.tabs.map((tab) => ({
				group: tab.group === null ? null : requireMappedId(groupIdMap, tab.group, 'Tab group'),
				id: requireMappedId(tabIdMap, tab.id, 'Tab'),
				pinned: tab.pinned,
				title: tab.title,
			})),
		});
		const panelWorkspaceState = clonePanelWorkspaceState(
			source.panelWorkspaceStore.getSnapshot(),
			tabIdMap,
			targetTerminalContextId,
			this.terminalPresentation.createPresentationId,
		);
		const sourceTabTitlePolicies = new Map(
			source.panelWorkspaceStore.getTabTitlePolicies().map((policy) => [policy.tabId, policy]),
		);
		const initialTabTitlePolicies: WorkbenchTabTitlePolicy[] = sourceSnapshot.tabs.map((tab) => {
			const policy = sourceTabTitlePolicies.get(tab.id);
			if (policy === undefined) throw new Error(`Tab ${tab.id} has no clone title policy.`);
			return {
				customTitle: policy.customTitle,
				fallbackTitle: policy.fallbackTitle,
				tabId: requireMappedId(tabIdMap, tab.id, 'Tab'),
			};
		});
		return {
			nextTabSequence: sourceSnapshot.tabs.length + 1,
			panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
				composition: this.terminalPresentation,
				initialState: panelWorkspaceState,
				initialTabTitlePolicies,
			}),
			tabModel,
			terminalContextId: targetTerminalContextId,
		};
	}

	private activeTerminalContextId(): string {
		return this.state.activeLocation.kind === 'standalone'
			? this.state.standaloneTerminalContextId
			: this.requireActiveWorkspaceTerminalContextId();
	}

	private getActiveMutableContext(): MutableWorkbenchTerminalContext {
		return this.requireContext(this.activeTerminalContextId());
	}

	private requireActiveWorkspaceTerminalContextId(): string {
		const activeLocation = this.state.activeLocation;
		if (activeLocation.kind !== 'workspace') {
			throw new Error('The active location is not a Workspace.');
		}
		const workspace = findWorkbenchWorkspace(this.state, activeLocation.workspaceId);
		if (workspace === undefined) {
			throw new Error('The active Workspace has no terminal context.');
		}
		return workspace.terminalContextId;
	}

	private requireContext(terminalContextId: string): MutableWorkbenchTerminalContext {
		const context = this.contexts.get(terminalContextId);
		if (context === undefined) {
			throw new Error(`Terminal context ${terminalContextId} does not exist.`);
		}
		return context;
	}

	private emit(): void {
		this.syncContextDocumentSubscriptions();
		for (const listener of this.listeners) {
			try {
				listener();
			} catch {
				// Observers cannot take ownership of or relabel an authoritative transition.
				continue;
			}
		}
		this.emitDocumentAndClientView();
	}

	private readonly emitDocumentAndClientView = (): void => {
		if (this.terminalTopologyListeners.size > 0) {
			const fingerprint = this.getTerminalTopologyRevision();
			if (fingerprint !== this.terminalTopologyFingerprint) {
				this.terminalTopologyFingerprint = fingerprint;
				for (const listener of this.terminalTopologyListeners) {
					try {
						listener();
					} catch {
						// Topology observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
		if (this.documentListeners.size > 0) {
			const fingerprint = sharedDocumentFingerprint(
				this.getSharedDocument(workbenchSharedDocumentRevision('0')),
			);
			if (fingerprint !== this.documentFingerprint) {
				this.documentFingerprint = fingerprint;
				for (const listener of this.documentListeners) {
					try {
						listener();
					} catch {
						// Document observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
		if (this.clientViewListeners.size > 0) {
			const fingerprint = JSON.stringify(this.getClientView());
			if (fingerprint !== this.clientViewFingerprint) {
				this.clientViewFingerprint = fingerprint;
				for (const listener of this.clientViewListeners) {
					try {
						listener();
					} catch {
						// Client View observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
	};

	private runtimeSnapshot() {
		return {
			projectWorkspace: this.state,
			terminalContexts: [...this.contexts].map(([terminalContextId, context]) => ({
				nextTabSequence: context.nextTabSequence,
				panelWorkspace: context.panelWorkspaceStore.getSnapshot(),
				tabCollection: context.tabModel.getSnapshot(),
				tabTitlePolicies: context.panelWorkspaceStore.getTabTitlePolicies(),
				terminalContextId,
			})),
		};
	}

	private syncContextDocumentSubscriptions(): void {
		if (
			this.documentListeners.size === 0 &&
			this.clientViewListeners.size === 0 &&
			this.terminalTopologyListeners.size === 0
		) {
			for (const unsubscribe of this.contextDocumentSubscriptions.values()) unsubscribe();
			this.contextDocumentSubscriptions.clear();
			return;
		}
		for (const [terminalContextId, unsubscribe] of this.contextDocumentSubscriptions) {
			if (this.contexts.has(terminalContextId)) continue;
			unsubscribe();
			this.contextDocumentSubscriptions.delete(terminalContextId);
		}
		for (const [terminalContextId, context] of this.contexts) {
			if (this.contextDocumentSubscriptions.has(terminalContextId)) continue;
			this.contextDocumentSubscriptions.set(
				terminalContextId,
				context.panelWorkspaceStore.subscribe(this.emitDocumentAndClientView),
			);
		}
	}
}

function requireMappedId(
	idMap: ReadonlyMap<string, string>,
	sourceId: string,
	label: string,
): string {
	const targetId = idMap.get(sourceId);
	if (targetId === undefined) throw new Error(`${label} ${sourceId} has no clone mapping.`);
	return targetId;
}

function canInsertTerminalTab(
	tabModel: TabModel,
	tabId: string,
	input: Pick<WorkbenchTerminalCreateOperationInput, 'fallbackTitle' | 'placement'>,
	activate: boolean,
): boolean {
	const requestedIndex = input.placement.index;
	const snapshot = tabModel.getSnapshot();
	if (
		requestedIndex !== undefined &&
		(!Number.isSafeInteger(requestedIndex) ||
			requestedIndex < 0 ||
			requestedIndex > snapshot.tabs.length)
	) {
		return false;
	}
	try {
		const candidate = new TabModel({
			activeTabId: snapshot.activeTabId,
			groups: snapshot.groups,
			mostRecentGroupId: snapshot.mostRecentGroupId,
			selectedTabIds: [...snapshot.selectedTabIds],
			selectionAnchorId: snapshot.selectionAnchorId,
			tabs: snapshot.tabs,
		});
		candidate.addTab(
			{
				group: input.placement.groupId,
				pinned: false,
				title: input.fallbackTitle,
			},
			{
				activate,
				id: tabId,
				...(requestedIndex === undefined ? {} : { index: requestedIndex }),
			},
		);
		return requestedIndex === undefined || candidate.getIndexOfTab(tabId) === requestedIndex;
	} catch {
		return false;
	}
}

function terminalLifecycleOperationId(
	lifecycle: TerminalPresentation['lifecycle'],
): TerminalPresentationOperationId | null {
	return 'operationId' in lifecycle ? lifecycle.operationId : null;
}

function createdTerminalIntent(
	lifecycle: TerminalPresentation['lifecycle'],
): WorkbenchTerminalCreateIntent {
	if (lifecycle.kind === 'preview') return { kind: 'preview' };
	if (lifecycle.kind === 'provisioning') {
		return { kind: 'provision', operationId: lifecycle.operationId };
	}
	throw new Error('A newly created Terminal must remain in its initial lifecycle.');
}

function removalIdForEvent(
	terminal: TerminalPresentation,
	event: TerminalPresentationEvent,
): TerminalPresentationRemovalId | null {
	if (event.type !== 'terminal-close-completed' && event.type !== 'terminal-close-failed') {
		return null;
	}
	return terminal.lifecycle.kind === 'closing' || terminal.lifecycle.kind === 'close-failed'
		? terminal.lifecycle.removalId
		: null;
}

function requireRemovalId(
	removalId: TerminalPresentationRemovalId | null,
): TerminalPresentationRemovalId {
	if (removalId === null) throw new Error('A terminal close result requires one removal identity.');
	return removalId;
}

function sharedDocumentFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function collectRuntimeTerminals(
	contexts: ReadonlyMap<string, MutableWorkbenchTerminalContext>,
): ReadonlyMap<string, TerminalPresentation> {
	const terminals = new Map<string, TerminalPresentation>();
	for (const context of contexts.values()) {
		for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
			for (const panel of collectPanelLeaves(tab.root)) {
				for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
					terminals.set(terminal.presentationId, terminal);
				}
			}
		}
	}
	return terminals;
}

function collectSharedPresentations(
	document: WorkbenchSharedDocument,
): ReadonlyMap<string, WorkbenchSharedPresentation> {
	const presentations = new Map<string, WorkbenchSharedPresentation>();
	const visit = (
		node: WorkbenchSharedDocument['terminalContexts'][number]['panelWorkspace']['tabs'][number]['root'],
	): void => {
		if (node.kind === 'panel') {
			for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
				presentations.set(presentation.presentationId, presentation);
			}
			return;
		}
		for (const child of node.children) visit(child.node);
	};
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) visit(tab.root);
	}
	return presentations;
}

function retainActiveLocation(
	previous: ProjectWorkspaceState['activeLocation'],
	next: ProjectWorkspaceState['activeLocation'],
): ProjectWorkspaceState['activeLocation'] {
	return workbenchActiveLocationsEqual(previous, next) ? previous : next;
}

/**
 * This Client's location outlives a publication unless its Workspace is gone, which selects the
 * first Workspace in display order. A Project it has not seen before starts expanded, even when
 * the view it starts from, such as a reloaded document's, already expands it.
 */
function publishedProjectWorkspaceState(
	previous: ProjectWorkspaceState,
	hydrated: ProjectWorkspaceState,
): ProjectWorkspaceState {
	const knownProjectIds = new Set(previous.projects.map((project) => project.id));
	const viewExpandedProjectIds = new Set(hydrated.expandedProjectIds);
	const firstWorkspace = firstWorkspaceInDisplayOrder(hydrated);
	return {
		...hydrated,
		activeLocation: retainActiveLocation(
			previous.activeLocation,
			hydrated.activeLocation.kind === 'standalone' && firstWorkspace !== undefined
				? { kind: 'workspace', workspaceId: firstWorkspace.id }
				: hydrated.activeLocation,
		),
		expandedProjectIds: hydrated.projects
			.map((project) => project.id)
			.filter(
				(projectId) => viewExpandedProjectIds.has(projectId) || !knownProjectIds.has(projectId),
			),
	};
}

/** The sidebar lists each Project's Workspaces in Project order, then Recents. */
function firstWorkspaceInDisplayOrder(
	state: ProjectWorkspaceState,
): WorkbenchWorkspace | undefined {
	for (const project of state.projects) {
		const workspace = state.workspaces.find((candidate) => candidate.projectId === project.id);
		if (workspace !== undefined) return workspace;
	}
	return state.workspaces.find((workspace) => workspace.projectId === null);
}

function publishedPanelWorkspaceState(
	state: PanelWorkspaceState,
	currentState: PanelWorkspaceState | undefined,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): PanelWorkspaceState {
	const currentTabs = new Map(currentState?.tabs.map((tab) => [tab.tabId, tab]) ?? []);
	return {
		...state,
		tabs: state.tabs.map((tab) => ({
			...tab,
			panelFocusHistory: survivingPanelFocusHistory(tab, currentTabs.get(tab.tabId)),
			root: publishedPanelNode(tab.root, observedTerminals, publication),
		})),
	};
}

function publishedPanelNode(
	node: PanelLayoutNode,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			...node,
			childPresentations: node.childPresentations.map((terminal) =>
				publishedTerminal(terminal, observedTerminals, publication),
			),
			primaryPresentation: publishedTerminal(
				node.primaryPresentation,
				observedTerminals,
				publication,
			),
		};
	}
	const weights = publication.splitWeights.get(node.id);
	if (weights === undefined) throw new Error(`Split ${node.id} was published without weights.`);
	return {
		...node,
		children: node.children.map((child, index) => ({
			...child,
			node: publishedPanelNode(child.node, observedTerminals, publication),
			weight: index === 0 ? weights[0] : weights[1],
		})),
	};
}

function publishedTerminal(
	hydrated: TerminalPresentation,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): TerminalPresentation {
	const published = publication.presentations.get(hydrated.presentationId);
	if (published === undefined) {
		throw new Error(`Terminal ${hydrated.presentationId} was published without its facts.`);
	}
	const observed = observedTerminals.get(hydrated.presentationId);
	return {
		...hydrated,
		...(observed === undefined
			? {}
			: {
					closeActivity: observed.closeActivity,
					currentCwd: observed.currentCwd,
					naturalTitle: observed.naturalTitle,
				}),
		...(published.customTitle === null ? {} : { customTitle: published.customTitle }),
		lifecycle: published.lifecycle,
	};
}

function survivingPanelFocusHistory(
	tab: PanelWorkspaceState['tabs'][number],
	currentTab: PanelWorkspaceState['tabs'][number] | undefined,
): readonly string[] {
	const livePanelIds = new Set(collectPanelLeaves(tab.root).map((panel) => panel.id));
	return [
		...(currentTab?.panelFocusHistory ?? []).filter(
			(panelId) => livePanelIds.has(panelId) && panelId !== tab.focusedPanelId,
		),
		tab.focusedPanelId,
	];
}

function preserveRuntimePanelWorkspaceState(
	state: PanelWorkspaceState,
	currentState: PanelWorkspaceState | undefined,
	currentTerminals: ReadonlyMap<string, TerminalPresentation>,
	currentPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
	incomingPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
): PanelWorkspaceState {
	const currentTabs = new Map(currentState?.tabs.map((tab) => [tab.tabId, tab]) ?? []);
	return {
		...state,
		tabs: state.tabs.map((tab) => ({
			...tab,
			panelFocusHistory: survivingPanelFocusHistory(tab, currentTabs.get(tab.tabId)),
			root: preservePanelNodeObservations(
				tab.root,
				currentTerminals,
				currentPresentations,
				incomingPresentations,
			),
		})),
	};
}

function preservePanelNodeObservations(
	node: PanelLayoutNode,
	currentTerminals: ReadonlyMap<string, TerminalPresentation>,
	currentPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
	incomingPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			...node,
			childPresentations: node.childPresentations.map((terminal) =>
				preserveTerminalObservation(
					terminal,
					currentTerminals,
					currentPresentations,
					incomingPresentations,
				),
			),
			primaryPresentation: preserveTerminalObservation(
				node.primaryPresentation,
				currentTerminals,
				currentPresentations,
				incomingPresentations,
			),
		};
	}
	return {
		...node,
		children: node.children.map((child) => ({
			...child,
			node: preservePanelNodeObservations(
				child.node,
				currentTerminals,
				currentPresentations,
				incomingPresentations,
			),
		})),
	};
}

function preserveTerminalObservation(
	hydrated: TerminalPresentation,
	currentTerminals: ReadonlyMap<string, TerminalPresentation>,
	currentPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
	incomingPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
): TerminalPresentation {
	const current = currentTerminals.get(hydrated.presentationId);
	const currentPresentation = currentPresentations.get(hydrated.presentationId);
	const incomingPresentation = incomingPresentations.get(hydrated.presentationId);
	return current !== undefined &&
		currentPresentation !== undefined &&
		incomingPresentation !== undefined &&
		JSON.stringify(currentPresentation) === JSON.stringify(incomingPresentation)
		? current
		: hydrated;
}
