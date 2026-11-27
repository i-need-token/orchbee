import type { WorkbenchPanelNode, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';

import type { TerminalSettingsMachines } from '../../terminal-settings-surface';
import {
	formatWorkbenchAgentLocation,
	formatWorkbenchAgentName,
	projectWorkbenchAgentNotification,
	WorkbenchAgentNotificationScheduler,
	type WorkbenchAgentNotificationCandidate,
	type WorkbenchAgentNotificationObserver,
	type WorkbenchAgentObservation,
	type WorkbenchAgentSource,
	type WorkbenchProjectWorkspaceRuntime,
} from '../../workbench';
import { ServerTerminalAgentSource } from '../../workbench-client';
import type { DesktopRemoteSession, DesktopRemoteSessions } from './DesktopRemoteSessions';
import { LOCAL_MACHINE } from './local-machine';

/** How long Quit All waits for the hub to take each machine's stop before it stops this computer's. */
const STOP_TAKEN_MILLISECONDS = 3000;

interface DesktopBackgroundSessionsOptions {
	readonly intl: () => IntlShape;
	readonly local: {
		readonly agents: WorkbenchAgentSource;
		readonly runtime: WorkbenchProjectWorkspaceRuntime;
	};
	readonly machines: TerminalSettingsMachines;
	readonly observer: WorkbenchAgentNotificationObserver;
	/** Shows a Workspace of a machine, `local` for this computer. */
	readonly open: (machine: string, workspaceId: string) => void;
	readonly remoteSessions: DesktopRemoteSessions;
	/** The machine on screen, null for this computer. */
	readonly shown: () => string | null;
}

/** What the other machines' connected Sessions hold, for the notification-area icon. */
export interface DesktopOtherSessionsStatus {
	readonly observations: readonly WorkbenchAgentObservation[];
	readonly runningTerminals: number;
}

interface Watched {
	readonly agents: ServerTerminalAgentSource;
	readonly remote: DesktopRemoteSession;
	readonly scheduler: WorkbenchAgentNotificationScheduler;
	readonly stop: () => void;
}

/**
 * The Sessions the window holds but does not show, as the TUI keeps its hidden machines: each one's
 * Agent notifications, scheduled under the shown Workbench's rules and opening its Workspace on its
 * machine, this computer's included while another machine is on screen; the other machines' counts
 * for the notification-area icon; and Quit All's stop of every connected machine's Session.
 */
export class DesktopBackgroundSessions {
	readonly #listeners = new Set<() => void>();
	readonly #options: DesktopBackgroundSessionsOptions;
	readonly #stops: (() => void)[] = [];
	readonly #watched = new Map<string, Watched>();

	constructor(options: DesktopBackgroundSessionsOptions) {
		this.#options = options;
		const local = options.local;
		const localScheduler = new WorkbenchAgentNotificationScheduler((candidate) => {
			if (options.shown() !== null) this.#notifyLocal(candidate);
		});
		const observeLocal = (): void => localScheduler.observe(local.agents.getSnapshot());
		this.#stops.push(
			local.agents.subscribe(observeLocal),
			() => localScheduler.reset(),
			options.remoteSessions.subscribe(() => this.#follow()),
			options.machines.subscribe(() => this.#follow()),
		);
		observeLocal();
		this.#follow();
	}

	/** The other machines' connected Sessions' Agents and running terminals. */
	status(): DesktopOtherSessionsStatus {
		const observations: WorkbenchAgentObservation[] = [];
		let runningTerminals = 0;
		const rows = this.#options.machines.getSnapshot()?.machines ?? [];
		for (const [machine, watched] of this.#watched) {
			observations.push(...watched.agents.getSnapshot().presentations.values());
			runningTerminals += rows.find((row) => row.machine === machine)?.terminals ?? 0;
		}
		return { observations, runningTerminals };
	}

	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	/**
	 * Quit All's part on the other machines: asks the hub to stop every online machine's Session, as
	 * Stop its terminals does, and resolves once the hub has taken each stop or a few seconds passed.
	 * The hub carries each stop out on its own, after the window has gone.
	 */
	async stopMachines(): Promise<void> {
		const { machines } = this.#options;
		const asked = (machines.getSnapshot()?.machines ?? [])
			.filter((row) => row.state === 'online' && !row.stopping)
			.map((row) => row.machine);
		for (const machine of asked) machines.request({ act: 'stop', machine });
		if (asked.length === 0) return;
		const taken = (): boolean => {
			const rows = machines.getSnapshot()?.machines ?? [];
			return asked.every((machine) => {
				const row = rows.find((candidate) => candidate.machine === machine);
				return row === undefined || row.stopping || row.state !== 'online';
			});
		};
		await new Promise<void>((resolve) => {
			const finish = (): void => {
				clearTimeout(timer);
				stop();
				resolve();
			};
			const timer = setTimeout(finish, STOP_TAKEN_MILLISECONDS);
			const stop = machines.subscribe(() => {
				if (taken()) finish();
			});
			if (taken()) finish();
		});
	}

	dispose(): void {
		for (const stop of this.#stops.splice(0)) stop();
		for (const machine of this.#watched.keys()) this.#unwatch(machine);
		this.#listeners.clear();
	}

	/** Watches every connected machine's Session, and lets go of each that ended. */
	#follow(): void {
		const { machines, remoteSessions } = this.#options;
		const connected = new Map<string, DesktopRemoteSession>();
		for (const row of machines.getSnapshot()?.machines ?? []) {
			const remote = remoteSessions.get(row.machine);
			if (remote !== undefined) connected.set(row.machine, remote);
		}
		let changed = false;
		for (const [machine, watched] of this.#watched) {
			if (connected.get(machine) === watched.remote) continue;
			this.#unwatch(machine);
			changed = true;
		}
		for (const [machine, remote] of connected) {
			if (this.#watched.has(machine)) continue;
			this.#watch(machine, remote);
			changed = true;
		}
		// A machine's terminal count changes with its row, so every publication is reported.
		if (changed || connected.size > 0) this.#notify();
	}

	#watch(machine: string, remote: DesktopRemoteSession): void {
		const agents = new ServerTerminalAgentSource(remote.connection, remote.session);
		const scheduler = new WorkbenchAgentNotificationScheduler((candidate) => {
			if (this.#options.shown() !== machine) this.#notifyRemote(machine, remote, candidate);
		});
		const observe = (): void => {
			scheduler.observe(agents.getSnapshot());
			this.#notify();
		};
		const stopAgents = agents.subscribe(observe);
		observe();
		this.#watched.set(machine, {
			agents,
			remote,
			scheduler,
			stop: () => {
				stopAgents();
				scheduler.reset();
				agents.dispose();
			},
		});
	}

	#unwatch(machine: string): void {
		this.#watched.get(machine)?.stop();
		this.#watched.delete(machine);
	}

	#notify(): void {
		for (const listener of this.#listeners) listener();
	}

	#notifyLocal(candidate: WorkbenchAgentNotificationCandidate): void {
		const { intl, local, observer, open } = this.#options;
		const notification = projectWorkbenchAgentNotification({
			candidate,
			intl: intl(),
			navigateAgent: (input) => open(LOCAL_MACHINE, input.workspaceId),
			runtime: local.runtime,
		});
		if (notification !== null) observer.observeAgentNotification(notification);
	}

	/** A notification about another machine's Agent, its Workspace named with `@` and the machine. */
	#notifyRemote(
		machine: string,
		remote: DesktopRemoteSession,
		candidate: WorkbenchAgentNotificationCandidate,
	): void {
		const { intl: currentIntl, machines, observer, open } = this.#options;
		const destination = findDestination(remote.session.snapshot, candidate.presentationId);
		if (destination === null) return;
		const intl = currentIntl();
		const label = machines.getSnapshot()?.machines.find((row) => row.machine === machine)?.label;
		const { agent, binding, sequence } = candidate.observation;
		observer.observeAgentNotification({
			key: `${machine}\0${candidate.presentationId}\0${binding}\0${sequence}`,
			notification: {
				body: intl.formatMessage(agentObservationMessages.notificationBody, {
					location: formatWorkbenchAgentLocation(intl, destination),
					workspace:
						label === undefined
							? destination.workspaceName
							: `${destination.workspaceName} @${label}`,
				}),
				onActivate: () => open(machine, destination.workspaceId),
				tag: `orchbee-agent:${machine}:${candidate.presentationId}`,
				title: intl.formatMessage(
					candidate.kind === 'done'
						? agentObservationMessages.notificationDoneTitle
						: agentObservationMessages.notificationRequestTitle,
					{ provider: formatWorkbenchAgentName(intl, agent) },
				),
			},
		});
	}
}

/** Where a Presentation of a Session's snapshot sits: its Workspace, Tab, Panel and terminal. */
function findDestination(
	snapshot: WorkbenchSnapshot,
	presentationId: string,
):
	| (Parameters<typeof formatWorkbenchAgentLocation>[1] & {
			readonly workspaceId: string;
			readonly workspaceName: string;
	  })
	| null {
	for (const context of snapshot.terminalContexts) {
		const workspace = snapshot.navigation.workspaces.find(
			(candidate) => candidate.terminalContextId === context.terminalContextId,
		);
		if (workspace === undefined) continue;
		for (const tab of context.tabs) {
			for (const [index, panel] of panelLeaves(tab.root).entries()) {
				const position =
					panel.primaryPresentation.presentationId === presentationId
						? ({ kind: 'primary' } as const)
						: childPosition(panel.childPresentations, presentationId);
				if (position === null) continue;
				return {
					panelPosition: index + 1,
					tabTitle: tab.label,
					terminalPosition: position,
					workspaceId: workspace.workspaceId,
					workspaceName: workspace.label,
				};
			}
		}
	}
	return null;
}

function childPosition(
	children: readonly { readonly presentationId: string }[],
	presentationId: string,
): { readonly kind: 'child'; readonly position: number } | null {
	const index = children.findIndex((child) => child.presentationId === presentationId);
	return index === -1 ? null : { kind: 'child', position: index + 1 };
}

/** A Tab's Panels in reading order. */
function panelLeaves(
	node: WorkbenchPanelNode,
): Extract<WorkbenchPanelNode, { readonly kind: 'panel' }>[] {
	if (node.kind === 'panel') return [node];
	return [...panelLeaves(node.children[0].node), ...panelLeaves(node.children[1].node)];
}
