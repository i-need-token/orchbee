import type { WorkbenchActiveLocation } from '../../project-workspace';
import {
	findPanel,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchAgentSnapshot } from '../types/workbench-agent-observation';
import {
	resolveWorkbenchAgentNavigationTarget,
	type WorkbenchAgentNavigationInput,
} from './project-workbench-agent-overview';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';
import { workbenchActiveLocationsEqual } from './workbench-terminal-location-activation';

type WorkbenchAgentNavigationProjection = 'pending' | 'ready' | 'unavailable';

interface WorkbenchAgentNavigationWaitingState {
	readonly actionAccepted: boolean;
	readonly input: WorkbenchAgentNavigationInput;
	readonly kind: 'waiting';
	readonly originLocation: WorkbenchActiveLocation;
	readonly projection: WorkbenchAgentNavigationProjection;
	readonly requestId: number;
}

export type WorkbenchAgentNavigationState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'idle' }
	| WorkbenchAgentNavigationWaitingState;

export type WorkbenchAgentNavigationEvent =
	| {
			readonly input: WorkbenchAgentNavigationInput;
			readonly kind: 'navigate';
			readonly originLocation: WorkbenchActiveLocation;
			readonly requestId: number;
	  }
	| {
			readonly kind: 'action-settled';
			readonly outcome: 'accepted' | 'rejected';
			readonly requestId: number;
	  }
	| {
			readonly activeLocation: WorkbenchActiveLocation;
			readonly kind: 'projection-observed';
			readonly projection: WorkbenchAgentNavigationProjection;
	  }
	| { readonly kind: 'dispose' }
	| { readonly kind: 'mount' };

export type WorkbenchAgentNavigationCommand =
	| {
			readonly input: WorkbenchAgentNavigationInput;
			readonly kind: 'invoke-agent-navigation';
			readonly requestId: number;
	  }
	| { readonly kind: 'announce-unavailable' }
	| { readonly kind: 'request-terminal-focus' };

interface WorkbenchAgentNavigationTransition {
	readonly commands: readonly WorkbenchAgentNavigationCommand[];
	readonly state: WorkbenchAgentNavigationState;
}

export const INITIAL_WORKBENCH_AGENT_NAVIGATION_STATE = {
	kind: 'disposed',
} as const satisfies WorkbenchAgentNavigationState;

export function transitionWorkbenchAgentNavigation(
	state: WorkbenchAgentNavigationState,
	event: WorkbenchAgentNavigationEvent,
): WorkbenchAgentNavigationTransition {
	if (event.kind === 'mount') {
		return state.kind === 'disposed'
			? { commands: [], state: { kind: 'idle' } }
			: { commands: [], state };
	}
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };

	if (event.kind === 'navigate') {
		return {
			commands: [
				{
					input: event.input,
					kind: 'invoke-agent-navigation',
					requestId: event.requestId,
				},
			],
			state: {
				actionAccepted: false,
				input: event.input,
				kind: 'waiting',
				originLocation: event.originLocation,
				projection: 'pending',
				requestId: event.requestId,
			},
		};
	}

	if (state.kind !== 'waiting') return { commands: [], state };
	if (event.kind === 'action-settled') {
		if (event.requestId !== state.requestId) return { commands: [], state };
		if (event.outcome === 'rejected') {
			return { commands: [{ kind: 'announce-unavailable' }], state: { kind: 'idle' } };
		}
		return state.projection === 'ready'
			? focusTransition()
			: { commands: [], state: { ...state, actionAccepted: true } };
	}

	if (event.projection === 'unavailable') {
		return { commands: [{ kind: 'announce-unavailable' }], state: { kind: 'idle' } };
	}
	if (event.projection === 'ready') {
		return state.actionAccepted
			? focusTransition()
			: { commands: [], state: { ...state, projection: 'ready' } };
	}
	if (
		!workbenchActiveLocationsEqual(event.activeLocation, state.originLocation) &&
		!isTargetWorkspace(event.activeLocation, state.input)
	) {
		return { commands: [], state: { kind: 'idle' } };
	}
	return { commands: [], state: { ...state, projection: 'pending' } };
}

export function resolveWorkbenchAgentNavigationProjection(
	input: WorkbenchAgentNavigationInput,
	snapshot: WorkbenchAgentSnapshot,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAgentNavigationProjection {
	const target = resolveWorkbenchAgentNavigationTarget(input, snapshot, runtime);
	if (target === null) return 'unavailable';
	const activeLocation = runtime.getSnapshot().activeLocation;
	if (activeLocation.kind !== 'workspace' || activeLocation.workspaceId !== target.workspaceId) {
		return 'pending';
	}
	const context = runtime.getTerminalContext(target.terminalContextId);
	if (context === null || context.tabModel.getSnapshot().activeTabId !== target.tabId) {
		return 'pending';
	}
	const panelTab = context.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === target.tabId);
	if (panelTab === undefined || panelTab.focusedPanelId !== target.panelId) return 'pending';
	const panel = findPanel(panelTab.root, target.panelId);
	return panel !== null && getDisplayedTerminal(panel).presentationId === target.presentationId
		? 'ready'
		: 'pending';
}

function focusTransition(): WorkbenchAgentNavigationTransition {
	return {
		commands: [{ kind: 'request-terminal-focus' }],
		state: { kind: 'idle' },
	};
}

function isTargetWorkspace(
	location: WorkbenchActiveLocation,
	input: WorkbenchAgentNavigationInput,
): boolean {
	return location.kind === 'workspace' && location.workspaceId === input.workspaceId;
}
