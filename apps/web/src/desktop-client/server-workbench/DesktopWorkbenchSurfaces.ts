import { createMemoryHistory, type RouterHistory } from '@tanstack/react-router';

import type { WebApplicationRouting } from '../../application-routing';
import type {
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../../terminal-settings-surface';
import type {
	WorkbenchMachineChangeRow,
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
import { LOCAL_MACHINE } from './local-machine';

/** A Session on screen: its runtime, the routing drawn from it, and which machine it is. */
export interface DesktopWorkbenchSurface {
	/** Null for this computer. */
	readonly machine: string | null;
	readonly routing: WebApplicationRouting;
	readonly runtime: ServerWorkbenchRuntime;
	readonly session: ServerWorkbenchSession;
}

interface DesktopWorkbenchSurfacesOptions {
	/** A saved machine's runtime, which `machine` names by its catalog ID. */
	readonly createRemoteRuntime: (
		machine: string,
		remote: DesktopRemoteSession,
	) => ServerWorkbenchRuntime;
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
 * duties, such as Settings, Update and the folder picker, stay with this computer's Session whatever
 * is shown; the Sessions not shown keep their notifications and counts in DesktopBackgroundSessions.
 */
export class DesktopWorkbenchSurfaces implements WorkbenchOtherMachines {
	readonly #listeners = new Set<() => void>();
	readonly #options: DesktopWorkbenchSurfacesOptions;
	readonly #stops: (() => void)[] = [];
	#key = '';
	/** The last landing of a move this window followed; undefined until the first view. */
	#landed: number | null | undefined;
	/** Ends the capacity reading a Change machine list holds open. */
	#releaseCapacity: (() => void) | null = null;
	/** The surface on screen whose connection ended, once seen. */
	#lost: DesktopWorkbenchSurface | undefined;
	/** A choice of a machine that is not connected, which lands once its connection opens. */
	#pending: Pending | null = null;
	#snapshot: WorkbenchOtherMachinesSnapshot = {
		card: null,
		change: null,
		machines: [],
		onScreen: null,
	};
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
				this.#followMove();
				refresh();
			}),
			remoteSessions.subscribe(refresh),
			local.session.subscribe(refresh),
		);
		this.#follow();
		refresh();
	}

	readonly getSnapshot = (): WorkbenchOtherMachinesSnapshot => this.#snapshot;

	/** The machine on screen, null for this computer. */
	shown(): string | null {
		return this.#surface?.machine ?? null;
	}

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

	/**
	 * Change machine: moves `workspaceId` of the Session on screen to `machine`. The machines process
	 * runs it as the TUI does, asks this window before it stops a program, and the window follows
	 * the Workspace to where it landed.
	 */
	readonly move = (workspaceId: string, workspaceName: string, machine: string): void => {
		this.#options.machines.request({
			act: 'move',
			from: this.#surface?.machine ?? null,
			label: workspaceName,
			to: machine === LOCAL_MACHINE ? null : machine,
			workspace: workspaceId,
		});
	};

	readonly watchCapacity = (watching: boolean): void => {
		if (watching) {
			this.#releaseCapacity ??= this.#options.machines.showCapacity();
		} else {
			this.#releaseCapacity?.();
			this.#releaseCapacity = null;
		}
	};

	dispose(): void {
		this.watchCapacity(false);
		for (const stop of this.#stops.splice(0)) stop();
		this.#unmount(this.#surface);
		this.#surface = undefined;
		this.#options.remoteSessions.dispose();
		this.#listeners.clear();
	}

	/** Shows `workspaceId`, or the Session's first location, of `machine` once its surface can. */
	#land(machine: string, workspaceId: string | null): boolean {
		const { local, remoteSessions } = this.#options;
		if (machine === LOCAL_MACHINE) {
			this.#mount(null, local.runtime, local.session, workspaceHistory(workspaceId));
			return true;
		}
		const remote = remoteSessions.get(machine);
		if (remote === undefined) return false;
		this.#mount(
			machine,
			this.#options.createRemoteRuntime(machine, remote),
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

	/** Follows a Workspace a move landed, once; a landing from before this window's first view is not. */
	#followMove(): void {
		const moved = this.#options.machines.getSnapshot()?.move.moved;
		if (moved === undefined) return;
		const landed = this.#landed;
		this.#landed = moved?.number ?? null;
		if (landed === undefined || moved === null || moved.number === landed) return;
		this.open(moved.machine ?? LOCAL_MACHINE, moved.workspace);
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
				machine: LOCAL_MACHINE,
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
		const change = view === null ? null : changeOf(view, shown);
		// Most publications change no name or membership, so the sidebar redraws only when one does.
		const key = JSON.stringify({ card, change, list, onScreen });
		if (key === this.#key) return;
		this.#key = key;
		this.#snapshot = { card, change, machines: list, onScreen };
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

/**
 * Change machine for a Workspace of the Session on `shown`'s machine (null for this computer), as
 * the TUI lists it: each machine's free capacity while it can take a Workspace, otherwise its state
 * word; a machine that cannot take one now is offered disabled.
 */
function changeOf(
	view: TerminalSettingsMachinesView,
	shown: string | null,
): NonNullable<WorkbenchOtherMachinesSnapshot['change']> {
	const here = shown ?? LOCAL_MACHINE;
	const rows: WorkbenchMachineChangeRow[] = [];
	for (const { band, machine } of view.change.rows) {
		const id = machine ?? LOCAL_MACHINE;
		const saved =
			machine === null ? undefined : view.machines.find((row) => row.machine === machine);
		if (machine !== null && saved === undefined) continue;
		rows.push({
			address: saved?.address ?? null,
			current: id === here,
			disabled: band === 'unusable',
			machine: id,
			name: saved?.label ?? view.change.labels.here,
			trailing: saved === undefined ? view.local.summary : trailingOf(saved),
		});
	}
	const { find, heading, manage, noMatch, note, title } = view.change.labels;
	return {
		labels: { find, heading, manage, noMatch, note, title },
		moving: view.move.pending !== null || view.move.question !== null,
		rows,
	};
}

/** What a saved machine's row ends with: its free capacity while it is online, else its state word. */
function trailingOf(machine: TerminalSettingsMachinesView['machines'][number]): string {
	if (machine.state !== 'online' || machine.stopping) return machine.word;
	return machine.capacity.summary;
}
