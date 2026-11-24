import { createMemoryHistory, type RouterHistory } from '@tanstack/react-router';

import type { WebApplicationRouting } from '../../application-routing';
import type { TerminalSettingsMachines } from '../../terminal-settings-surface';
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
	#snapshot: WorkbenchOtherMachinesSnapshot = { machines: [], onScreen: null };
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
	 * Shows `workspaceId` of `machine`. A saved machine's surface replaces the one on screen only
	 * while its connection is open; otherwise nothing changes.
	 */
	readonly open = (machine: string, workspaceId: string): void => {
		const history = createMemoryHistory({
			initialEntries: [`/workspaces/${encodeURIComponent(workspaceId)}`],
		});
		const { local, remoteSessions } = this.#options;
		if (machine === LOCAL) {
			this.#mount(null, local.runtime, local.session, history);
			return;
		}
		const remote = remoteSessions.get(machine);
		if (remote === undefined) return;
		this.#mount(machine, this.#options.createRemoteRuntime(remote), remote.session, history);
	};

	dispose(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.#unmount(this.#surface);
		this.#surface = undefined;
		this.#options.remoteSessions.dispose();
		this.#listeners.clear();
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

	#refresh(): void {
		const { local, machines, remoteSessions } = this.#options;
		// The surface on screen whose connection ended returns to this computer.
		const surface = this.#surface;
		if (
			surface?.machine !== undefined &&
			surface.machine !== null &&
			remoteSessions.get(surface.machine)?.session !== surface.session
		) {
			this.#mount(null, local.runtime, local.session, createMemoryHistory());
			return;
		}
		const shown = surface?.machine ?? null;
		const rows = machines.getSnapshot()?.machines ?? [];
		const list: WorkbenchOtherMachineWorkspaces[] = [];
		if (shown !== null) {
			list.push({
				leading: true,
				machine: LOCAL,
				mark: null,
				...serverNavigationRows(local.session.snapshot),
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
			if (remote === undefined) continue;
			list.push({
				leading: !passed,
				machine: row.machine,
				mark,
				...serverNavigationRows(remote.session.snapshot),
			});
		}
		// Most publications change no name or membership, so the sidebar redraws only when one does.
		const key = JSON.stringify({ list, onScreen });
		if (key === this.#key) return;
		this.#key = key;
		this.#snapshot = { machines: list, onScreen };
		for (const listener of this.#listeners) listener();
	}
}
