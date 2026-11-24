import { createMemoryHistory, type RouterHistory } from '@tanstack/react-router';

import type { WebApplicationRouting } from '../../application-routing';
import type {
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../../terminal-settings-surface';
import type {
	WorkbenchOtherMachines,
	WorkbenchOtherMachinesSnapshot,
	WorkbenchOtherMachineWorkspaces,
} from '../../workbench';
import {
	serverNavigationRows,
	type ServerWorkbenchRuntime,
	type ServerWorkbenchSession,
} from '../../workbench-client';
import type { DesktopRemoteSession, DesktopRemoteSessions } from './DesktopRemoteSessions';

/** This computer, as the other machines' list names it; saved machines are named by catalog ID. */
const LOCAL = 'local';

/** A Session on screen: its runtime, the routing drawn from it, and which machine it is. */
export interface DesktopWorkbenchSurface {
	/** Null for this computer. */
	readonly machine: string | null;
	readonly routing: WebApplicationRouting;
	readonly runtime: ServerWorkbenchRuntime;
	readonly session: ServerWorkbenchSession;
}

interface DesktopWorkbenchSurfacesOptions {
	readonly createRemoteRuntime: (remote: DesktopRemoteSession) => ServerWorkbenchRuntime;
	readonly createRouting: (
		runtime: ServerWorkbenchRuntime,
		local: boolean,
		otherMachines: WorkbenchOtherMachines,
		history: RouterHistory,
	) => WebApplicationRouting;
	readonly local: {
		readonly runtime: ServerWorkbenchRuntime;
		readonly session: ServerWorkbenchSession;
	};
	readonly machines: TerminalSettingsMachines;
	readonly remoteSessions: DesktopRemoteSessions;
	readonly render: (surface: DesktopWorkbenchSurface) => void;
}

/**
 * Which Session the window shows, and the other machines' Workspaces its sidebar lists beside it.
 * This computer's runtime lasts the document's life, so returning to it never waits; a saved
 * machine's runtime lasts while it is on screen, over the connection the window keeps to it. Window
 * duties, such as Settings, Update, Quit All and the notification-area counts, stay with this
 * computer's Session whatever is shown.
 */
export class DesktopWorkbenchSurfaces implements WorkbenchOtherMachines {
	readonly #listeners = new Set<() => void>();
	readonly #options: DesktopWorkbenchSurfacesOptions;
	readonly #stops: (() => void)[] = [];
	#key = '';
	/** The surface on screen whose connection ended, once seen. */
	#lost: DesktopWorkbenchSurface | undefined;
	/** A choice of a machine that is not connected, which lands once its connection opens. */
	#pending: Pending | null = null;
	#snapshot: WorkbenchOtherMachinesSnapshot = { card: null, machines: [], onScreen: null };
	#surface: DesktopWorkbenchSurface | undefined;

	constructor(options: DesktopWorkbenchSurfacesOptions) {
		this.#options = options;
	}

	/** Shows this computer's Session and starts following the saved machines. */
	start(): void {
		const { local, machines, remoteSessions } = this.#options;
		this.#mount(null, local.runtime, local.session, createMemoryHistory());
		const refresh = (): void => this.#refresh();
		this.#stops.push(
			machines.subscribe(() => {
				this.#follow();
				refresh();
			}),
			remoteSessions.subscribe(refresh),
			local.session.subscribe(refresh),
		);
		this.#follow();
		refresh();
	}

	readonly getSnapshot = (): WorkbenchOtherMachinesSnapshot => this.#snapshot;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	/**
	 * Shows `workspaceId` of `machine`. A saved machine whose connection is not open is chosen as
	 * the TUI chooses it: its state's action runs, so the hub tries at once and a login asks this
	 * window, and the card stands over the terminals until the connection opens and the choice
	 * lands, another choice is made, or Not now gives it up.
	 */
	readonly open = (machine: string, workspaceId: string): void => {
		this.#pending = null;
		if (this.#land(machine, workspaceId)) return;
		this.#pending = { dismissed: null, machine, reattach: false, workspaceId };
		this.act();
		this.#refresh();
	};

	readonly act = (): void => {
		const pending = this.#pending;
		if (pending === null) return;
		const action = this.#machine(pending.machine)?.action;
		// Disconnect is the button of a machine already online; its connection is on its way.
		if (action === undefined || action === null || action.act === 'disconnect') return;
		this.#options.machines.request({ act: action.act, machine: pending.machine });
	};

	/**
	 * Gives up the person's choice. A lost surface's return stays armed, as the TUI's does: its card
	 * comes back when the machine's state changes, and the surface returns once it can.
	 */
	readonly dismiss = (): void => {
		const pending = this.#pending;
		this.#pending =
			pending?.reattach === true
				? { ...pending, dismissed: this.#machine(pending.machine)?.state ?? null }
				: null;
		this.#refresh();
	};

	dispose(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.#unmount(this.#surface);
		this.#surface = undefined;
		this.#options.remoteSessions.dispose();
		this.#listeners.clear();
	}

	/** Shows `workspaceId`, or the Session's first location, of `machine` once its surface can. */
	#land(machine: string, workspaceId: string | null): boolean {
		const { local, remoteSessions } = this.#options;
		if (machine === LOCAL) {
			this.#mount(null, local.runtime, local.session, workspaceHistory(workspaceId));
			return true;
		}
		const remote = remoteSessions.get(machine);
		if (remote === undefined) return false;
		this.#mount(
			machine,
			this.#options.createRemoteRuntime(remote),
			remote.session,
			workspaceHistory(workspaceId),
		);
		return true;
	}

	#mount(
		machine: string | null,
		runtime: ServerWorkbenchRuntime,
		session: ServerWorkbenchSession,
		history: RouterHistory,
	): void {
		const routing = this.#options.createRouting(runtime, machine === null, this, history);
		const previous = this.#surface;
		this.#surface = { machine, routing, runtime, session };
		this.#refresh();
		this.#options.render(this.#surface);
		this.#unmount(previous);
	}

	/** A saved machine's runtime ends with its surface; this computer's stays. */
	#unmount(surface: DesktopWorkbenchSurface | undefined): void {
		if (surface === undefined) return;
		surface.routing.dispose();
		if (surface.machine !== null) surface.runtime.dispose();
	}

	/** Connects every machine the hub reads online. */
	#follow(): void {
		const { machines, remoteSessions } = this.#options;
		remoteSessions.follow(
			new Set(
				(machines.getSnapshot()?.machines ?? [])
					.filter((machine) => machine.state === 'online')
					.map((machine) => machine.machine),
			),
		);
	}

	#machine(machine: string): TerminalSettingsMachinesView['machines'][number] | undefined {
		return this.#options.machines.getSnapshot()?.machines.find((row) => row.machine === machine);
	}

	#refresh(): void {
		const { local, machines, remoteSessions } = this.#options;
		const surface = this.#surface;
		// The surface on screen whose connection ended stays beneath the card and returns to the
		// Workspace it showed once its machine's connection opens again; a machine no longer saved
		// returns the window to this computer.
		if (
			surface !== undefined &&
			surface.machine !== null &&
			remoteSessions.get(surface.machine)?.session !== surface.session
		) {
			if (this.#machine(surface.machine) === undefined) {
				this.#mount(null, local.runtime, local.session, createMemoryHistory());
				return;
			}
			if (surface !== this.#lost) {
				this.#lost = surface;
				this.#pending = {
					dismissed: null,
					machine: surface.machine,
					reattach: true,
					workspaceId: shownWorkspace(surface),
				};
			}
		}
		const pending = this.#pending;
		if (pending !== null && remoteSessions.get(pending.machine) !== undefined) {
			this.#pending = null;
			this.#land(pending.machine, pending.workspaceId);
			return;
		}
		const shown = surface?.machine ?? null;
		const view = machines.getSnapshot();
		const rows = view?.machines ?? [];
		const seenOf = (machine: string) => view?.seen.find((entry) => entry.machine === machine);
		const list: WorkbenchOtherMachineWorkspaces[] = [];
		if (shown !== null) {
			list.push({
				leading: true,
				machine: LOCAL,
				mark: null,
				...serverNavigationRows(local.session.snapshot),
				word: null,
			});
		}
		let passed = shown === null;
		let onScreen: WorkbenchOtherMachinesSnapshot['onScreen'] = null;
		for (const row of rows) {
			const mark = { color: row.color, name: row.label };
			if (row.machine === shown) {
				passed = true;
				onScreen = mark;
				continue;
			}
			const remote = remoteSessions.get(row.machine);
			// A machine that is not connected keeps the Workspaces it listed when last read.
			const seen = remote === undefined ? seenOf(row.machine) : undefined;
			if (remote === undefined && seen === undefined) continue;
			list.push({
				leading: !passed,
				machine: row.machine,
				mark,
				...(remote === undefined
					? { projects: seen?.projects ?? [], word: row.word, workspaces: seen?.workspaces ?? [] }
					: { ...serverNavigationRows(remote.session.snapshot), word: null }),
			});
		}
		const chosen =
			pending === null ? undefined : rows.find((row) => row.machine === pending.machine);
		if (pending !== null && chosen === undefined) this.#pending = null;
		const card =
			chosen === undefined || chosen.state === pending?.dismissed
				? null
				: {
						action:
							chosen.action === null || chosen.action.act === 'disconnect'
								? null
								: chosen.action.label,
						...chosen.card,
					};
		// Most publications change no name or membership, so the sidebar redraws only when one does.
		const key = JSON.stringify({ card, list, onScreen });
		if (key === this.#key) return;
		this.#key = key;
		this.#snapshot = { card, machines: list, onScreen };
		for (const listener of this.#listeners) listener();
	}
}

/**
 * A chosen machine's Workspace that waits for its connection. A lost surface's return is `reattach`;
 * Not now on it records the state it was given at, and its card stays away while that state holds.
 */
interface Pending {
	readonly dismissed: string | null;
	readonly machine: string;
	readonly reattach: boolean;
	readonly workspaceId: string | null;
}

/** A routing history that starts on `workspaceId`, or on the Session's first location. */
function workspaceHistory(workspaceId: string | null): RouterHistory {
	return workspaceId === null
		? createMemoryHistory()
		: createMemoryHistory({ initialEntries: [`/workspaces/${encodeURIComponent(workspaceId)}`] });
}

/** The Workspace `surface` shows, or null on any other location. */
function shownWorkspace(surface: DesktopWorkbenchSurface): string | null {
	const match = /^\/workspaces\/([^/]+)/u.exec(surface.routing.router.state.location.pathname);
	const encoded = match?.[1];
	if (encoded === undefined) return null;
	try {
		return decodeURIComponent(encoded);
	} catch {
		return null;
	}
}
