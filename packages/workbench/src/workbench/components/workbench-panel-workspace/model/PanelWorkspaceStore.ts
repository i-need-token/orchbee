import type { TabModel, TabModelSnapshot } from '@orchbee/tab-strip';

import type {
	TerminalPresentationCloseRequest,
	TerminalPresentationCreation,
	TerminalPresentationId,
} from '../../../../terminal-presentation';
import type { WorkbenchTerminalPresentationComposition } from '../../../model/createWorkbenchPreviewTerminalPresentationComposition';
import {
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelWorkspaceEvent,
	type PanelWorkspaceState,
	type PanelWorkspaceTransition,
} from './panel-workspace-state';

type Listener = () => void;

interface PanelWorkspaceStoreOptions {
	readonly composition: WorkbenchTerminalPresentationComposition;
	readonly initialCwd?: string | null | undefined;
	readonly initialPresentationId?: TerminalPresentationId | undefined;
	readonly initialState?: PanelWorkspaceState | undefined;
}

export class PanelWorkspaceStore {
	private readonly listeners = new Set<Listener>();
	private readonly pendingTabCwds = new Map<string, string | null>();
	private readonly pendingTabPresentations = new Map<string, TerminalPresentationCreation>();
	private readonly composition: WorkbenchTerminalPresentationComposition;
	private readonly tabModel: TabModel;
	private unsubscribeTabModel: (() => void) | null = null;
	private state: PanelWorkspaceState;

	public constructor(tabModel: TabModel, options: PanelWorkspaceStoreOptions) {
		this.tabModel = tabModel;
		this.composition = options.composition;
		if (options.initialState !== undefined) {
			this.state = options.initialState;
			return;
		}
		const snapshot = tabModel.getSnapshot();
		const initialTabs = snapshot.tabs.map((tab, index) => ({
			cwd: options.initialCwd ?? null,
			id: tab.id,
			presentation: this.createPresentation(
				index === 0
					? (options.initialPresentationId ?? options.composition.initialPresentationId)
					: undefined,
			),
			title: tab.title,
		}));
		const initialization = initializePanelWorkspace(initialTabs);
		this.state = initialization.state;
		this.dispatchCommands(initialization.commands);
	}

	public readonly getSnapshot = (): PanelWorkspaceState => this.state;
	public readonly getActiveTabId = (): string | null => this.tabModel.getSnapshot().activeTabId;

	public readonly subscribe = (listener: Listener): (() => void) => {
		if (this.unsubscribeTabModel === null) {
			this.unsubscribeTabModel = this.tabModel.subscribe(this.reconcileTabs);
			this.reconcileTabs();
		}
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
			if (this.listeners.size === 0) {
				this.unsubscribeTabModel?.();
				this.unsubscribeTabModel = null;
			}
		};
	};

	public transition(event: PanelWorkspaceEvent): PanelWorkspaceTransition {
		const result = transitionPanelWorkspace(this.state, event);
		if (result.state !== this.state) {
			this.state = result.state;
			this.emit();
		}
		this.dispatchCommands(result.commands);
		return result;
	}

	public createTerminalPresentation(): TerminalPresentationCreation {
		return this.createPresentation();
	}

	public createTerminalCloseRequest(
		presentationIds: readonly TerminalPresentationId[],
	): TerminalPresentationCloseRequest {
		if (this.composition.kind === 'preview') return { kind: 'preview' };
		const composition = this.composition;
		return {
			kind: 'managed',
			operations: presentationIds.map((presentationId) => ({
				operationId: composition.createOperationId(),
				presentationId,
			})),
			removalId: composition.createRemovalId(),
		};
	}

	public prepareTabCwd(tabId: string, cwd: string | null): void {
		this.pendingTabCwds.set(tabId, cwd);
		this.pendingTabPresentations.set(tabId, this.createPresentation());
	}

	public readonly reconcileTabs = (): void => {
		const result = this.transition({
			tabs: this.toTabInputs(this.tabModel.getSnapshot()),
			type: 'tabs-reconciled',
		});
		if (result.state === this.state && result.outcome === 'unchanged') {
			this.emit();
		}
		for (const tab of result.state.tabs) {
			this.pendingTabCwds.delete(tab.tabId);
			this.pendingTabPresentations.delete(tab.tabId);
		}
	};

	private emit(): void {
		for (const listener of this.listeners) {
			listener();
		}
	}

	private createPresentation(
		presentationId?: TerminalPresentationId,
	): TerminalPresentationCreation {
		const resolvedPresentationId = presentationId ?? this.composition.createPresentationId();
		return this.composition.kind === 'preview'
			? { kind: 'preview', presentationId: resolvedPresentationId }
			: {
					kind: 'managed',
					operationId: this.composition.createOperationId(),
					presentationId: resolvedPresentationId,
				};
	}

	private dispatchCommands(commands: PanelWorkspaceTransition['commands']): void {
		if (commands.length > 0 && this.composition.kind === 'managed') {
			this.composition.dispatchCommands(commands);
		}
	}

	private toTabInputs(snapshot: TabModelSnapshot, initialCwd?: string | null) {
		const existingTabIds = new Set(this.state.tabs.map((tab) => tab.tabId));
		return snapshot.tabs.map((tab) => {
			let presentation = this.pendingTabPresentations.get(tab.id);
			if (!existingTabIds.has(tab.id) && presentation === undefined) {
				presentation = this.createPresentation();
				this.pendingTabPresentations.set(tab.id, presentation);
			}
			return {
				cwd: this.pendingTabCwds.get(tab.id) ?? initialCwd ?? null,
				id: tab.id,
				...(presentation === undefined ? {} : { presentation }),
				title: tab.title,
			};
		});
	}
}
