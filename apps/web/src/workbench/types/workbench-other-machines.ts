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

export interface WorkbenchOtherMachinesSnapshot {
	readonly card: WorkbenchMachineCard | null;
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
	open(machine: string, workspaceId: string): void;
	subscribe(listener: () => void): () => void;
}
