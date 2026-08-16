import { TabModel, type TabStripNewTabRequest } from '@orchbee/tab-strip';

import type {
	TerminalPresentationEvent,
	TerminalPresentationFailure,
	TerminalPresentationId,
	TerminalPresentationRemovalId,
} from '../../terminal-presentation';
import {
	workbenchSharedDocumentRevision,
	type WorkbenchClientView,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
	type WorkbenchSharedPresentation,
} from '../document';
import {
	createProjectWorkspaceState,
	resolveTerminalCwd,
	transitionProjectWorkspace,
	type ProjectWorkspaceEvent,
	type ProjectWorkspaceState,
	type ProjectWorkspaceTransition,
	type ResolveTerminalCwdResult,
	type TerminalContextCommand,
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
} from './workbench-document-runtime';
import type { WorkbenchTerminalMetadataObservation } from './resolveTerminalMetadataObservation';

type Listener = () => void;

export interface WorkbenchTerminalContext {
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly tabModel: TabModel;
}

interface MutableWorkbenchTerminalContext extends WorkbenchTerminalContext {
	nextTabSequence: number;
}

export interface WorkbenchProjectWorkspaceRuntimeOptions {
	readonly initialClientView?: WorkbenchClientView | undefined;
	readonly initialDocument?: WorkbenchSharedDocument | undefined;
	readonly standaloneCwd: string;
	readonly standalonePresentations?: readonly WorkbenchStandaloneTerminalPresentation[] | undefined;
	readonly standaloneTerminalTitle: string;
	readonly terminalPresentation: WorkbenchTerminalPresentationComposition;
}

export interface WorkbenchStandaloneTerminalPresentation {
	readonly cwd: string;
	readonly presentationId: TerminalPresentationId;
	readonly title: string;
}

export type WorkbenchTerminalPresentationEventResult =
	| { readonly kind: 'applied' | 'unchanged' }
	| { readonly kind: 'rejected'; readonly reason: string };

export type WorkbenchSharedDocumentApplyResult =
	| { readonly kind: 'applied'; readonly clientView: WorkbenchClientView }
	| { readonly kind: 'unchanged' };

export interface WorkbenchRuntimeTerminalCreateInput extends TabStripNewTabRequest {
	readonly directoryId?: string | undefined;
	readonly title: string;
}

export type WorkbenchRuntimeTerminalCreateResult =
	| {
			readonly kind: 'created';
			readonly tabId: string;
	  }
	| Extract<ResolveTerminalCwdResult, { kind: 'rejected' }>
	| { readonly kind: 'rejected'; readonly reason: 'terminal-creation-sealed' };

export interface WorkbenchTerminalSessionStopFailure {
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
	private readonly systemCwd: string;
	private readonly terminalPresentation: WorkbenchTerminalPresentationComposition;
	private terminalCreationSealed = false;
	private state = createProjectWorkspaceState();

	public constructor(options: WorkbenchProjectWorkspaceRuntimeOptions) {
		this.systemCwd = options.standaloneCwd;
		this.terminalPresentation = options.terminalPresentation;
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
		const currentTerminals = collectRuntimeTerminals(this.contexts);
		const currentPresentations = collectSharedPresentations(projected);
		const incomingPresentations = collectSharedPresentations(document);
		for (const unsubscribe of this.contextDocumentSubscriptions.values()) unsubscribe();
		this.contextDocumentSubscriptions.clear();
		this.contexts.clear();
		this.state = hydrated.projectWorkspace;
		for (const terminalContext of hydrated.terminalContexts) {
			const tabModel = new TabModel(terminalContext.tabCollection);
			this.contexts.set(terminalContext.terminalContextId, {
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
					composition: this.terminalPresentation,
					initialState: preserveRuntimeTerminalObservations(
						terminalContext.panelWorkspace,
						currentTerminals,
						currentPresentations,
						incomingPresentations,
					),
					initialTabTitlePolicies: terminalContext.tabTitlePolicies,
				}),
				tabModel,
			});
		}
		this.emit();
		return { clientView: hydrated.clientView, kind: 'applied' };
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
							case 'stopping':
								commands.push({
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

	public resolveActiveTerminalCwd(directoryId?: string): ResolveTerminalCwdResult {
		if (this.state.activeLocation.kind === 'standalone') {
			return resolveTerminalCwd({
				directories: [],
				...(directoryId === undefined ? {} : { explicitDirectoryId: directoryId }),
				systemCwd: this.systemCwd,
			});
		}
		const activeLocation = this.state.activeLocation;
		const project = this.state.projects.find(
			(candidate) => candidate.id === activeLocation.projectId,
		);
		const workspace = project?.workspaces.find(
			(candidate) => candidate.id === activeLocation.workspaceId,
		);
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
		const matches: Array<{ readonly store: PanelWorkspaceStore; readonly tabId: string }> = [];
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				if (findTerminal(tab.root, presentationId) !== null) {
					matches.push({ store: context.panelWorkspaceStore, tabId: tab.tabId });
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
			closeActivity: observation.closeActivity,
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
	): ProjectWorkspaceTransition {
		const result = transitionProjectWorkspace(this.state, event);
		if (result.outcome !== 'applied') return result;
		if (
			this.terminalCreationSealed &&
			result.commands.some(
				(command) =>
					command.type === 'terminal-context-created' || command.type === 'terminal-context-cloned',
			)
		) {
			throw new Error('Terminal context creation is sealed for application shutdown.');
		}
		beforeCommit?.();
		for (const command of result.commands) this.applyCommand(command);
		this.state = result.state;
		this.emit();
		return result;
	}

	public createTerminal(
		input: WorkbenchRuntimeTerminalCreateInput,
	): WorkbenchRuntimeTerminalCreateResult {
		if (this.terminalCreationSealed) {
			return { kind: 'rejected', reason: 'terminal-creation-sealed' };
		}
		const cwdResult = this.resolveActiveTerminalCwd(input.directoryId);
		if (cwdResult.kind === 'rejected') return cwdResult;
		const context = this.getActiveMutableContext();
		const tabId = `${this.activeTerminalContextId()}-tab-${context.nextTabSequence}`;
		context.nextTabSequence += 1;
		context.panelWorkspaceStore.prepareTabCwd(tabId, cwdResult.cwd);
		context.tabModel.addTab(
			{
				group: input.groupId,
				pinned: false,
				title: input.title,
			},
			{
				id: tabId,
				...(input.index === undefined ? {} : { index: input.index }),
			},
		);
		context.panelWorkspaceStore.reconcileTabs();
		return { kind: 'created', tabId };
	}

	private applyCommand(command: TerminalContextCommand): void {
		switch (command.type) {
			case 'terminal-context-created':
				this.contexts.set(
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
				this.contexts.set(
					command.targetTerminalContextId,
					this.cloneTerminalContext(
						command.sourceTerminalContextId,
						command.targetTerminalContextId,
					),
				);
				break;
			case 'terminal-context-disposed':
				this.contexts.delete(command.terminalContextId);
				break;
		}
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
		const project = this.state.projects.find(
			(candidate) => candidate.id === activeLocation.projectId,
		);
		const workspace = project?.workspaces.find(
			(candidate) => candidate.id === activeLocation.workspaceId,
		);
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
		if (this.documentListeners.size === 0 && this.clientViewListeners.size === 0) {
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

function preserveRuntimeTerminalObservations(
	state: PanelWorkspaceState,
	currentTerminals: ReadonlyMap<string, TerminalPresentation>,
	currentPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
	incomingPresentations: ReadonlyMap<string, WorkbenchSharedPresentation>,
): PanelWorkspaceState {
	return {
		...state,
		tabs: state.tabs.map((tab) => ({
			...tab,
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
