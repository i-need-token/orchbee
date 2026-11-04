import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import {
	matchesWorkbenchProjectCreation,
	type WorkbenchNavigationPort,
	type WorkbenchNavigationTarget,
	type WorkbenchProjectCreationReceipt,
	type WorkbenchProjectWorkspaceRuntime,
} from '../workbench';

import {
	canonicalWorkbenchRoute,
	resolveWorkbenchRoute,
	workbenchRouteLocationExists,
	workbenchRouteLocationsEqual,
	type WorkbenchRouteLocation,
	type WorkbenchRouteRequest,
	type WorkbenchRouteResolution,
	type WorkbenchRouteSnapshot,
} from './workbench-route-model';

type WorkbenchTabSelectionInput = Parameters<WorkbenchNavigationPort['selectTab']>[1];
type WorkbenchTabModel = NonNullable<
	ReturnType<WorkbenchProjectWorkspaceRuntime['getActiveTerminalContext']>
>['tabModel'];
type WorkbenchRouteNavigationResult = 'navigated' | 'unavailable' | 'unchanged' | 'updated';

type WorkbenchRouteNavigation = (
	location: WorkbenchRouteLocation,
	options: { readonly replace: boolean },
) => void;

const PROJECT_CREATION_ROUTE_ACCEPTANCE_TIMEOUT_MS = 10_000;

interface PendingProjectCreationRoute {
	readonly location: WorkbenchRouteLocation;
	readonly receipt: WorkbenchProjectCreationReceipt;
	settle(result: ActionResult<void>): void;
}

function readWorkbenchRouteSnapshot(
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchRouteSnapshot {
	const state = runtime.getSnapshot();
	return {
		activeLocation: state.activeLocation,
		projects: state.projects.map((project) => ({ id: project.id })),
		workspaces: state.workspaces.map((workspace) => {
			const context = runtime.getTerminalContext(workspace.terminalContextId);
			if (context === null) throw new Error(`Workspace ${workspace.id} has no terminal context.`);
			const tabs = context.tabModel.getSnapshot();
			return {
				activeTabId: tabs.activeTabId,
				id: workspace.id,
				projectId: workspace.projectId,
				tabIds: tabs.tabs.map((tab) => tab.id),
			};
		}),
	};
}

export class WorkbenchRouteCoordinator {
	#applying = false;
	#disposed = false;
	#displayedLocation: WorkbenchRouteLocation | undefined;
	readonly #navigate: WorkbenchRouteNavigation;
	#pendingProjectCreation: PendingProjectCreationRoute | undefined;
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
		isActive: (target) => {
			const displayed = this.#displayedLocation;
			return (
				!this.#disposed &&
				displayed !== undefined &&
				(this.#requestedLocation === undefined ||
					workbenchRouteLocationsEqual(this.#requestedLocation, displayed)) &&
				displayed.kind === 'workspace' &&
				displayed.workspaceId === target.workspaceId
			);
		},
		navigate: (context, target, receipt) => this.#navigateFromAction(context, target, receipt),
		selectTab: (context, input) => this.#navigateTabFromAction(context, input),
	};

	public constructor(
		runtime: WorkbenchProjectWorkspaceRuntime,
		navigate: WorkbenchRouteNavigation,
	) {
		this.#runtime = runtime;
		this.#navigate = navigate;
		this.#unsubscribe = runtime.subscribeClientView(this.#handleClientViewChange);
	}

	public dispose(): void {
		this.#disposed = true;
		this.#rejectPendingProjectCreation('route-acceptance-retired');
		this.#unsubscribe();
		this.#displayedLocation = undefined;
		this.#pendingSelection = undefined;
		this.#requestedLocation = undefined;
	}

	public prepare(request: WorkbenchRouteRequest): WorkbenchRouteResolution {
		const resolution = resolveWorkbenchRoute(readWorkbenchRouteSnapshot(this.#runtime), request);
		if (resolution.kind === 'ready') {
			this.#applyRoute(resolution.location);
		} else if (resolution.kind === 'unavailable') {
			this.#rejectPendingProjectCreation('route-target-unavailable');
			this.#displayedLocation = undefined;
			this.#pendingSelection = undefined;
			this.#requestedLocation = undefined;
		} else if (
			this.#pendingProjectCreation !== undefined &&
			!workbenchRouteLocationsEqual(this.#pendingProjectCreation.location, resolution.location)
		) {
			this.#rejectPendingProjectCreation('route-acceptance-replaced');
		}
		return resolution;
	}

	#applyRoute(location: WorkbenchRouteLocation): void {
		const pending = this.#pendingProjectCreation;
		this.#applying = true;
		try {
			const activeLocation = this.#runtime.getSnapshot().activeLocation;
			if (location.kind === 'no-workspace') {
				// The runtime opens No Workspace by itself, and only once no Workspace is left.
				if (activeLocation.kind !== 'no-workspace') {
					throw new Error('No Workspace cannot be shown while a Workspace is open.');
				}
			} else {
				if (
					activeLocation.kind !== 'workspace' ||
					activeLocation.workspaceId !== location.workspaceId
				) {
					const transition = this.#runtime.transition({
						type: 'workspace-activated',
						workspaceId: location.workspaceId,
					});
					if (transition.outcome === 'rejected') {
						throw new Error('The Workspace route could not be activated.');
					}
				}
				this.#applyWorkspaceTab(location);
			}
			this.#displayedLocation = location;
			this.#pendingSelection = undefined;
			this.#requestedLocation = undefined;
		} finally {
			this.#applying = false;
		}
		if (pending !== undefined) {
			if (!workbenchRouteLocationsEqual(pending.location, location)) {
				pending.settle({ code: 'route-acceptance-replaced', kind: 'blocked' });
			} else if (this.#matchesProjectCreation(pending.receipt)) {
				pending.settle({ kind: 'completed' });
			} else {
				pending.settle({ code: 'project-creation-target-replaced', kind: 'blocked' });
			}
		}
	}

	#applyWorkspaceTab(
		location: Extract<WorkbenchRouteLocation, { readonly kind: 'workspace' }>,
	): void {
		const tabModel = this.#requireActiveTabModel();
		if (location.tabId === null) {
			if (tabModel.getSnapshot().tabs.length !== 0) {
				throw new Error('An empty-context route cannot display a non-empty Tab collection.');
			}
			return;
		}
		const pendingSelection = this.#pendingSelection;
		this.#applyTabSelection(
			tabModel,
			pendingSelection !== undefined &&
				workbenchRouteLocationsEqual(pendingSelection.location, location)
				? pendingSelection.input
				: { mode: 'replace', tabId: location.tabId },
		);
	}

	#requireActiveTabModel(): WorkbenchTabModel {
		const context = this.#runtime.getActiveTerminalContext();
		if (context === null) throw new Error('No Workspace is open.');
		return context.tabModel;
	}

	#applyTabSelection(tabModel: WorkbenchTabModel, input: WorkbenchTabSelectionInput): boolean {
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
		this.#requestNavigation(
			canonical,
			!workbenchRouteLocationExists(snapshot, this.#displayedLocation),
		);
	};

	#locationForTarget(target: WorkbenchNavigationTarget): WorkbenchRouteLocation | null {
		const snapshot = readWorkbenchRouteSnapshot(this.#runtime);
		const workspace = snapshot.workspaces.find((candidate) => candidate.id === target.workspaceId);
		return workspace === undefined
			? null
			: {
					kind: 'workspace',
					tabId: workspace.activeTabId ?? workspace.tabIds[0] ?? null,
					workspaceId: workspace.id,
				};
	}

	#navigateFromAction(
		context: ActionExecutionContext,
		target: WorkbenchNavigationTarget,
		creationReceipt?: WorkbenchProjectCreationReceipt,
	): ActionResult<void> | Promise<ActionResult<void>> {
		if (context.signal.aborted) return { kind: 'cancelled' };
		this.#rejectPendingProjectCreation('route-acceptance-replaced');
		if (creationReceipt !== undefined)
			return this.#navigateCreatedProject(context, target, creationReceipt);
		const result = this.#navigateTo(target, context.markCommitted);
		return navigationActionResult(result);
	}

	#matchesProjectCreation(receipt: WorkbenchProjectCreationReceipt): boolean {
		return matchesWorkbenchProjectCreation(this.#runtime, receipt);
	}

	#rejectPendingProjectCreation(code: string): void {
		this.#pendingProjectCreation?.settle({ code, kind: 'blocked' });
	}

	#navigateCreatedProject(
		context: ActionExecutionContext,
		target: WorkbenchNavigationTarget,
		receipt: WorkbenchProjectCreationReceipt,
	): ActionResult<void> | Promise<ActionResult<void>> {
		if (this.#disposed) return { code: 'route-acceptance-retired', kind: 'blocked' };
		if (target.workspaceId !== receipt.workspaceId || !this.#matchesProjectCreation(receipt))
			return { code: 'project-creation-target-replaced', kind: 'blocked' };
		const location = this.#locationForTarget(target);
		if (location === null) return { code: 'route-target-unavailable', kind: 'blocked' };
		if (
			this.navigation.isActive?.(target) === true &&
			this.#displayedLocation !== undefined &&
			workbenchRouteLocationsEqual(this.#displayedLocation, location)
		)
			return { kind: 'unchanged' };
		return new Promise<ActionResult<void>>((resolve) => {
			const retire = () => pending.settle({ code: 'route-acceptance-retired', kind: 'blocked' });
			const timeout = setTimeout(
				() => pending.settle({ code: 'route-acceptance-timeout', kind: 'blocked' }),
				PROJECT_CREATION_ROUTE_ACCEPTANCE_TIMEOUT_MS,
			);
			const pending: PendingProjectCreationRoute = {
				location,
				receipt,
				settle: (result) => {
					if (this.#pendingProjectCreation !== pending) return;
					this.#pendingProjectCreation = undefined;
					clearTimeout(timeout);
					context.signal.removeEventListener('abort', retire);
					resolve(result);
				},
			};
			this.#pendingProjectCreation = pending;
			context.signal.addEventListener('abort', retire);
			if (context.signal.aborted) {
				retire();
				return;
			}
			try {
				// History dispatch commits navigation, but only prepare accepts its exact route.
				context.markCommitted();
				if (this.#pendingProjectCreation !== pending || context.signal.aborted) return;
				this.#requestNavigation(location, false);
			} catch {
				pending.settle({ code: 'route-target-unavailable', kind: 'blocked' });
			}
		});
	}

	#navigateTabFromAction(
		context: ActionExecutionContext,
		input: WorkbenchTabSelectionInput,
	): ActionResult<void> {
		if (context.signal.aborted) return { kind: 'cancelled' };
		this.#rejectPendingProjectCreation('route-acceptance-replaced');
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
		const tabModel = this.#runtime.getActiveTerminalContext()?.tabModel;
		if (activeLocation.kind !== 'workspace' || tabModel === undefined) return 'unavailable';
		const tabSnapshot = tabModel.getSnapshot();
		if (!tabSnapshot.tabs.some((tab) => tab.id === input.tabId)) return 'unavailable';
		const targetTabId =
			input.mode === 'toggle' && tabSnapshot.selectedTabIds.has(input.tabId)
				? tabSnapshot.activeTabId
				: input.tabId;
		if (targetTabId === null) return 'unavailable';
		const workspace = snapshot.workspaces.find(
			(candidate) => candidate.id === activeLocation.workspaceId,
		);
		if (workspace === undefined || !workspace.tabIds.includes(targetTabId)) {
			return 'unavailable';
		}
		const location: WorkbenchRouteLocation = {
			kind: 'workspace',
			tabId: targetTabId,
			workspaceId: workspace.id,
		};
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
			this.#pendingProjectCreation !== undefined &&
			!workbenchRouteLocationsEqual(this.#pendingProjectCreation.location, location)
		)
			this.#rejectPendingProjectCreation('route-acceptance-replaced');
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
