/** A saved machine's color, the same names Settings › Machines uses. */
type WorkbenchMachineColor = 'cyan' | 'grey' | 'pink' | 'purple';

/** The machine a sidebar row names after `@`. */
export interface WorkbenchMachineMark {
	readonly color: WorkbenchMachineColor;
	readonly name: string;
}

/** One other machine's Workspaces, which the sidebar lists beside the Session on screen. */
export interface WorkbenchOtherMachineWorkspaces {
	/** Whether its rows come before the Session on screen's in each group: this computer first, then saved order. */
	readonly leading: boolean;
	readonly machine: string;
	/** Null for this computer, whose rows carry no mark. */
	readonly mark: WorkbenchMachineMark | null;
	readonly projects: readonly { readonly id: string; readonly name: string }[];
	readonly workspaces: readonly {
		readonly id: string;
		readonly name: string;
		readonly projectId: string | null;
	}[];
	/** The state word of a machine that is not connected, whose rows are those it listed when last read; null while connected. */
	readonly word: string | null;
}

/** What stands over the terminals while a chosen machine is not connected, worded by the host. */
interface WorkbenchMachineCard {
	/** The state's action, which chooses the machine again, or null when it has none. */
	readonly action: string | null;
	readonly lines: readonly string[];
	readonly notNow: string;
	readonly title: string;
}

/** One row of Change machine's list. */
export interface WorkbenchMachineChangeRow {
	readonly address: string | null;
	/** The Workspace runs there now; choosing it changes nothing. */
	readonly current: boolean;
	/** It cannot take a Workspace now. */
	readonly disabled: boolean;
	/** `local` for this computer, otherwise the saved machine. */
	readonly machine: string;
	readonly name: string;
	/** What it has free, or its state word. */
	readonly trailing: string;
}

/** Change machine in a Workspace's menu: its words and where the Workspace can run, worded by the host. */
interface WorkbenchMachineChange {
	readonly labels: {
		readonly find: string;
		readonly heading: string;
		readonly manage: string;
		readonly noMatch: string;
		readonly note: string;
		readonly title: string;
	};
	/** A move runs, so the list offers no machine. */
	readonly moving: boolean;
	/** In the order the host lists them; the sidebar keeps the order taken when the list opens. */
	readonly rows: readonly WorkbenchMachineChangeRow[];
}

export interface WorkbenchOtherMachinesSnapshot {
	readonly card: WorkbenchMachineCard | null;
	/** Null where the host offers no Change machine. */
	readonly change: WorkbenchMachineChange | null;
	readonly machines: readonly WorkbenchOtherMachineWorkspaces[];
	/** The mark of the machine on screen, null while this computer's Session shows. */
	readonly onScreen: WorkbenchMachineMark | null;
}

/**
 * The Workspaces of the machines whose Sessions are not on screen, where a window holds more than
 * one Session. Choosing one shows it once its machine's surface can; until then the card stands
 * over the terminals.
 */
export interface WorkbenchOtherMachines {
	/** The card's action. */
	act(): void;
	/** The card's Not now: the choice is given up and the terminals beneath stay. */
	dismiss(): void;
	getSnapshot(): WorkbenchOtherMachinesSnapshot;
	/** Change machine: moves a Workspace of the Session on screen to `machine`. */
	move(workspaceId: string, workspaceName: string, machine: string): void;
	open(machine: string, workspaceId: string): void;
	subscribe(listener: () => void): () => void;
	/** Whether a Change machine list is open, which alone reads what the machines have free. */
	watchCapacity(watching: boolean): void;
}
