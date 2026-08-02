import { TabModel, type TabStripNewTabRequest } from '@orchbee/tab-strip';

import type {
	TerminalPresentationEvent,
	TerminalPresentationId,
	TerminalPresentationRemovalId,
} from '../../terminal-presentation';
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
import {
	clonePanelWorkspaceState,
	findTerminal,
	type TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchTerminalPresentationComposition } from './createWorkbenchPreviewTerminalPresentationComposition';

type Listener = () => void;

export interface WorkbenchTerminalContext {
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly tabModel: TabModel;
}

interface MutableWorkbenchTerminalContext extends WorkbenchTerminalContext {
	nextTabSequence: number;
}

export interface WorkbenchProjectWorkspaceRuntimeOptions {
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

export interface WorkbenchRuntimeTerminalCreateInput extends TabStripNewTabRequest {
	readonly directoryId?: string | undefined;
	readonly title: string;
}

export type WorkbenchRuntimeTerminalCreateResult =
	| {
			readonly kind: 'created';
			readonly tabId: string;
	  }
	| Extract<ResolveTerminalCwdResult, { kind: 'rejected' }>;

export class WorkbenchProjectWorkspaceRuntime {
	private readonly contexts = new Map<string, MutableWorkbenchTerminalContext>();
	private readonly listeners = new Set<Listener>();
	private readonly systemCwd: string;
	private readonly terminalPresentation: WorkbenchTerminalPresentationComposition;
	private state = createProjectWorkspaceState();

	public constructor(options: WorkbenchProjectWorkspaceRuntimeOptions) {
		this.systemCwd = options.standaloneCwd;
		this.terminalPresentation = options.terminalPresentation;
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

	public readonly subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

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
		const removalId = removalIdForEvent(match.terminal, event);
		if (
			(event.type === 'terminal-close-completed' || event.type === 'terminal-close-failed') &&
			removalId === null
		) {
			return { kind: 'rejected', reason: 'stale-operation' };
		}
		const transition = match.store.transition(
			event.type === 'terminal-close-completed' || event.type === 'terminal-close-failed'
				? { ...event, removalId: requireRemovalId(removalId), tabId: match.tabId }
				: { ...event, tabId: match.tabId },
		);
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
		beforeCommit?.();
		for (const command of result.commands) this.applyCommand(command);
		this.state = result.state;
		this.emit();
		return result;
	}

	public createTerminal(
		input: WorkbenchRuntimeTerminalCreateInput,
	): WorkbenchRuntimeTerminalCreateResult {
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
		return {
			nextTabSequence: sourceSnapshot.tabs.length + 1,
			panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
				composition: this.terminalPresentation,
				initialState: panelWorkspaceState,
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
		for (const listener of this.listeners) {
			try {
				listener();
			} catch {
				// Observers cannot take ownership of or relabel an authoritative transition.
				continue;
			}
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
