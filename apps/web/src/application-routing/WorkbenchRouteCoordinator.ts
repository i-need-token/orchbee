import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import {
	type WorkbenchNavigationPort,
	type WorkbenchNavigationTarget,
	type WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';

import {
	canonicalWorkbenchRoute,
	resolveWorkbenchRoute,
	workbenchRouteLocationsEqual,
	type WorkbenchRouteLocation,
	type WorkbenchRouteRequest,
	type WorkbenchRouteResolution,
	type WorkbenchRouteSnapshot,
} from './workbench-route-model';
import {
	transitionInitialTerminalContext,
	type InitialTerminalContextEvent,
	type InitialTerminalContextState,
	type InitialTerminalContextTransition,
} from './transitionInitialTerminalContext';

type WorkbenchTabSelectionInput = Parameters<WorkbenchNavigationPort['selectTab']>[1];
type WorkbenchRouteNavigationResult = 'navigated' | 'unavailable' | 'unchanged' | 'updated';

type WorkbenchRouteNavigation = (
	location: WorkbenchRouteLocation,
	options: { readonly replace: boolean },
) => void;

function readWorkbenchRouteSnapshot(
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchRouteSnapshot {
	const state = runtime.getSnapshot();
	const standalone = runtime.getTerminalContext(state.standaloneTerminalContextId);
	if (standalone === null) throw new Error('The standalone terminal context is unavailable.');
	const standaloneTabs = standalone.tabModel.getSnapshot();
	return {
		activeLocation: state.activeLocation,
		projects: state.projects.map((project) => ({
			id: project.id,
			workspaces: project.workspaces.map((workspace) => {
				const context = runtime.getTerminalContext(workspace.terminalContextId);
				if (context === null) {
					throw new Error(`Workspace ${workspace.id} has no terminal context.`);
				}
				const tabs = context.tabModel.getSnapshot();
				return {
					activeTabId: tabs.activeTabId,
					id: workspace.id,
					tabIds: tabs.tabs.map((tab) => tab.id),
				};
			}),
		})),
		standalone: {
			activeTabId: standaloneTabs.activeTabId,
			tabIds: standaloneTabs.tabs.map((tab) => tab.id),
		},
	};
}

export class WorkbenchRouteCoordinator {
	#applying = false;
	#displayedLocation: WorkbenchRouteLocation | undefined;
	#initialTerminalContextState: InitialTerminalContextState = 'awaiting-route';
	readonly #initialTerminalTitle: string;
	readonly #navigate: WorkbenchRouteNavigation;
	#pendingSelection:
		| {
				readonly input: WorkbenchTabSelectionInput;
				readonly location: WorkbenchRouteLocation;
		  }
		| undefined;
	#requestedLocation: WorkbenchRouteLocation | undefined;
	readonly #runtime: WorkbenchProjectWorkspaceRuntime;
	readonly #unsubscribe: () => void;

	public readonly navigation: WorkbenchNavigationPort = {
		navigate: (context, target) => this.#navigateFromAction(context, target),
		selectTab: (context, input) => this.#navigateTabFromAction(context, input),
	};

	public constructor(
		runtime: WorkbenchProjectWorkspaceRuntime,
		navigate: WorkbenchRouteNavigation,
		initialTerminalTitle: string,
	) {
		this.#runtime = runtime;
		this.#navigate = navigate;
		this.#initialTerminalTitle = initialTerminalTitle;
		this.#unsubscribe = runtime.subscribeClientView(this.#handleClientViewChange);
	}

	public dispose(): void {
		this.#unsubscribe();
		this.#displayedLocation = undefined;
		this.#pendingSelection = undefined;
		this.#requestedLocation = undefined;
	}

	public navigateTo(target: WorkbenchNavigationTarget): WorkbenchRouteNavigationResult {
		return this.#navigateTo(target);
	}

	public navigateToTab(input: WorkbenchTabSelectionInput): WorkbenchRouteNavigationResult {
		return this.#navigateToTab(input);
	}

	public prepare(request: WorkbenchRouteRequest): WorkbenchRouteResolution {
		const resolution = resolveWorkbenchRoute(readWorkbenchRouteSnapshot(this.#runtime), request);
		if (resolution.kind === 'ready') {
			this.#applyRoute(resolution.location);
			const commands = this.#transitionInitialTerminalContext({
				hasTabs: resolution.location.tabId !== null,
				type: 'terminal-route-ready',
			});
			if (commands.length === 0) return resolution;

			this.#applying = true;
			let creation: ReturnType<WorkbenchProjectWorkspaceRuntime['createTerminal']>;
			try {
				creation = this.#runtime.createTerminal({
					groupId: null,
					source: 'programmatic',
					title: this.#initialTerminalTitle,
				});
			} finally {
				this.#applying = false;
			}
			if (creation.kind === 'rejected') return resolution;

			const createdResolution = resolveWorkbenchRoute(
				readWorkbenchRouteSnapshot(this.#runtime),
				request,
			);
			if (createdResolution.kind === 'ready') this.#applyRoute(createdResolution.location);
			return createdResolution;
		} else if (resolution.kind === 'unavailable') {
			this.#displayedLocation = undefined;
			this.#pendingSelection = undefined;
			this.#requestedLocation = undefined;
			this.#transitionInitialTerminalContext({ type: 'route-unavailable' });
		} else {
			this.#transitionInitialTerminalContext({ type: 'redirect-observed' });
		}
		return resolution;
	}

	#transitionInitialTerminalContext(
		event: InitialTerminalContextEvent,
	): InitialTerminalContextTransition['commands'] {
		const transition = transitionInitialTerminalContext(this.#initialTerminalContextState, event);
		this.#initialTerminalContextState = transition.state;
		return transition.commands;
	}

	#applyRoute(location: WorkbenchRouteLocation): void {
		this.#applying = true;
		try {
			const activeLocation = this.#runtime.getSnapshot().activeLocation;
			if (location.kind === 'standalone') {
				if (activeLocation.kind !== 'standalone') {
					const transition = this.#runtime.transition({ type: 'standalone-activated' });
					if (transition.outcome === 'rejected') {
						throw new Error('The standalone route could not be activated.');
					}
				}
			} else if (
				activeLocation.kind !== 'workspace' ||
				activeLocation.projectId !== location.projectId ||
				activeLocation.workspaceId !== location.workspaceId
			) {
				const transition = this.#runtime.transition({
					projectId: location.projectId,
					type: 'workspace-activated',
					workspaceId: location.workspaceId,
				});
				if (transition.outcome === 'rejected') {
					throw new Error('The Workspace route could not be activated.');
				}
			}

			const tabModel = this.#runtime.getActiveTerminalContext().tabModel;
			if (location.tabId === null) {
				if (tabModel.getSnapshot().tabs.length !== 0) {
					throw new Error('An empty-context route cannot display a non-empty Tab collection.');
				}
			} else {
				const pendingSelection = this.#pendingSelection;
				this.#applyTabSelection(
					tabModel,
					pendingSelection !== undefined &&
						workbenchRouteLocationsEqual(pendingSelection.location, location)
						? pendingSelection.input
						: { mode: 'replace', tabId: location.tabId },
				);
			}
			this.#displayedLocation = location;
			this.#pendingSelection = undefined;
			this.#requestedLocation = undefined;
		} finally {
			this.#applying = false;
		}
	}

	#applyTabSelection(
		tabModel: ReturnType<WorkbenchProjectWorkspaceRuntime['getActiveTerminalContext']>['tabModel'],
		input: WorkbenchTabSelectionInput,
	): boolean {
		const snapshot = tabModel.getSnapshot();
		if (
			input.mode === 'replace' &&
			snapshot.activeTabId === input.tabId &&
			snapshot.selectedTabIds.size === 1
		) {
			return false;
		}
		switch (input.mode) {
			case 'add-range':
				tabModel.addSelectionFromAnchorTo(input.tabId);
				break;
			case 'extend':
				tabModel.extendSelectionTo(input.tabId);
				break;
			case 'replace':
				tabModel.selectTab(input.tabId);
				break;
			case 'toggle':
				tabModel.toggleSelected(input.tabId);
				break;
		}
		return tabModel.getSnapshot().revision !== snapshot.revision;
	}

	readonly #handleClientViewChange = (): void => {
		if (this.#applying || this.#displayedLocation === undefined) return;
		const snapshot = readWorkbenchRouteSnapshot(this.#runtime);
		const canonical = canonicalWorkbenchRoute(snapshot);
		if (workbenchRouteLocationsEqual(this.#displayedLocation, canonical)) return;
		this.#requestNavigation(canonical, !locationExists(snapshot, this.#displayedLocation));
	};

	#locationForTarget(target: WorkbenchNavigationTarget): WorkbenchRouteLocation | null {
		const snapshot = readWorkbenchRouteSnapshot(this.#runtime);
		if (target.kind === 'standalone') {
			return {
				kind: 'standalone',
				tabId: snapshot.standalone.activeTabId ?? snapshot.standalone.tabIds[0] ?? null,
			};
		}
		const project = snapshot.projects.find((candidate) => candidate.id === target.projectId);
		const workspace = project?.workspaces.find((candidate) => candidate.id === target.workspaceId);
		return project === undefined || workspace === undefined
			? null
			: {
					kind: 'workspace',
					projectId: project.id,
					tabId: workspace.activeTabId ?? workspace.tabIds[0] ?? null,
					workspaceId: workspace.id,
				};
	}

	#navigateFromAction(
		context: ActionExecutionContext,
		target: WorkbenchNavigationTarget,
	): ActionResult<void> {
		if (context.signal.aborted) return { kind: 'cancelled' };
		const result = this.#navigateTo(target, context.markCommitted);
		return navigationActionResult(result);
	}

	#navigateTabFromAction(
		context: ActionExecutionContext,
		input: WorkbenchTabSelectionInput,
	): ActionResult<void> {
		if (context.signal.aborted) return { kind: 'cancelled' };
		const result = this.#navigateToTab(input, context.markCommitted);
		return navigationActionResult(result);
	}

	#navigateTo(
		target: WorkbenchNavigationTarget,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchRouteNavigationResult {
		const location = this.#locationForTarget(target);
		if (location === null) return 'unavailable';
		if (
			this.#displayedLocation !== undefined &&
			workbenchRouteLocationsEqual(this.#displayedLocation, location)
		) {
			return 'unchanged';
		}
		beforeCommit?.();
		this.#requestNavigation(location, false);
		return 'navigated';
	}

	#navigateToTab(
		input: WorkbenchTabSelectionInput,
		beforeCommit?: (() => void) | undefined,
	): WorkbenchRouteNavigationResult {
		const snapshot = readWorkbenchRouteSnapshot(this.#runtime);
		const activeLocation = snapshot.activeLocation;
		const tabModel = this.#runtime.getActiveTerminalContext().tabModel;
		const tabSnapshot = tabModel.getSnapshot();
		if (!tabSnapshot.tabs.some((tab) => tab.id === input.tabId)) return 'unavailable';
		const targetTabId =
			input.mode === 'toggle' && tabSnapshot.selectedTabIds.has(input.tabId)
				? tabSnapshot.activeTabId
				: input.tabId;
		if (targetTabId === null) return 'unavailable';
		let location: WorkbenchRouteLocation;
		if (activeLocation.kind === 'standalone') {
			location = { kind: 'standalone', tabId: targetTabId };
		} else {
			const project = snapshot.projects.find(
				(candidate) => candidate.id === activeLocation.projectId,
			);
			const workspace = project?.workspaces.find(
				(candidate) => candidate.id === activeLocation.workspaceId,
			);
			if (
				project === undefined ||
				workspace === undefined ||
				!workspace.tabIds.includes(targetTabId)
			) {
				return 'unavailable';
			}
			location = {
				kind: 'workspace',
				projectId: project.id,
				tabId: targetTabId,
				workspaceId: workspace.id,
			};
		}
		beforeCommit?.();
		if (
			this.#displayedLocation !== undefined &&
			workbenchRouteLocationsEqual(this.#displayedLocation, location)
		) {
			this.#applying = true;
			try {
				return this.#applyTabSelection(tabModel, input) ? 'updated' : 'unchanged';
			} finally {
				this.#applying = false;
			}
		}
		this.#pendingSelection = { input, location };
		this.#requestNavigation(location, false);
		return 'navigated';
	}

	#requestNavigation(location: WorkbenchRouteLocation, replace: boolean): void {
		if (
			this.#requestedLocation !== undefined &&
			workbenchRouteLocationsEqual(this.#requestedLocation, location)
		) {
			return;
		}
		this.#requestedLocation = location;
		this.#navigate(location, { replace });
	}
}

function locationExists(
	snapshot: WorkbenchRouteSnapshot,
	location: WorkbenchRouteLocation,
): boolean {
	if (location.kind === 'standalone') {
		return location.tabId === null || snapshot.standalone.tabIds.includes(location.tabId);
	}
	const project = snapshot.projects.find((candidate) => candidate.id === location.projectId);
	const workspace = project?.workspaces.find((candidate) => candidate.id === location.workspaceId);
	return (
		workspace !== undefined &&
		(location.tabId === null || workspace.tabIds.includes(location.tabId))
	);
}

function navigationActionResult(result: WorkbenchRouteNavigationResult): ActionResult<void> {
	switch (result) {
		case 'navigated':
			return { kind: 'completed' };
		case 'unavailable':
			return { code: 'route-target-unavailable', kind: 'blocked' };
		case 'unchanged':
			return { kind: 'unchanged' };
		case 'updated':
			return { kind: 'completed' };
	}
}
