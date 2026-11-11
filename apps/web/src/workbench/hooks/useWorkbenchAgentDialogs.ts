import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentBoard } from '../types/workbench-agent-board';
import type {
	WorkbenchAgentCatalogSource,
	WorkbenchAgentStartPlace,
} from '../types/workbench-agent-start';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import { subscribeToNothing } from '../utils/subscribeToNothing';

interface UseWorkbenchAgentDialogsOptions {
	/** Absent where the authority cannot read the Server's Agent sessions; the board then never opens. */
	readonly agentBoard: WorkbenchAgentBoard | undefined;
	/** Absent where the authority publishes no Agent catalog; Start Agent then never opens. */
	readonly agentCatalog: WorkbenchAgentCatalogSource | undefined;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/** What Start Agent shows while it is open. */
export interface WorkbenchAgentStartDialogModel {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly finalFocus: () => HTMLElement | false | null;
	/** The places a start can use now, in the dialog's order; the first is the default. */
	readonly places: readonly WorkbenchAgentStartPlace[];
}

/** What the Agent board shows while it is open. */
export interface WorkbenchAgentBoardDialogModel {
	readonly board: WorkbenchAgentBoard;
	readonly finalFocus: () => HTMLElement | false | null;
}

interface WorkbenchAgentDialogs {
	/** Present while the board is open. */
	readonly board: WorkbenchAgentBoardDialogModel | undefined;
	readonly closeBoard: () => void;
	/**
	 * Closes Start Agent: back to the board when Start Agent replaced it, otherwise to where the
	 * keyboard was.
	 */
	readonly closeStart: () => void;
	/** A start ran: Start Agent closes on this frame, with the board it replaced. */
	readonly endStart: () => void;
	readonly openBoard: () => void;
	/**
	 * Opens Start Agent with the places `workbench.agent.start` found available. Opened while the
	 * board is open, it replaces the board, which comes back when Start Agent closes.
	 */
	readonly openStart: (places: readonly WorkbenchAgentStartPlace[]) => void;
	/** Closes the board after a jump and hands the shown terminal the keyboard. */
	readonly showBoardSession: () => void;
	/** Present while Start Agent is open. */
	readonly start: WorkbenchAgentStartDialogModel | undefined;
	/** Whether the Session can start an Agent now, which offers Start Agent's entry points. */
	readonly startOffered: boolean;
}

type Shown =
	| { readonly kind: 'board' }
	| { readonly kind: 'none' }
	| {
			readonly fromBoard: boolean;
			readonly kind: 'start';
			readonly places: readonly WorkbenchAgentStartPlace[];
	  };

const NONE: Shown = { kind: 'none' };
const BOARD: Shown = { kind: 'board' };

/**
 * The one owner of the Agent board and Start Agent, which never stack: Start Agent opened from the
 * board takes its place, as the TUI's picker takes the board's, and closing it brings the board
 * back. Focus returns once, when the last of them closes, to where it was before the first opened;
 * passing from one to the other moves it only into the dialog that opens. A jump from the board or a
 * start hands the keyboard to the terminal it shows instead.
 */
export function useWorkbenchAgentDialogs({
	agentBoard,
	agentCatalog,
	beginTerminalHandover,
	projectWorkspaceRuntime,
}: UseWorkbenchAgentDialogsOptions): WorkbenchAgentDialogs {
	const [shown, setShown] = React.useState<Shown>(NONE);
	// Read by the closing dialog's final focus, after the change it caused has been decided.
	const next = React.useRef<Shown>(NONE);
	const returnFocus = React.useRef<HTMLElement | null>(null);
	const landed = React.useRef(false);
	const readStartable = () => agentCatalog?.getSnapshot()?.startable === true;
	const startOffered = React.useSyncExternalStore(
		agentCatalog?.subscribe ?? subscribeToNothing,
		readStartable,
		readStartable,
	);

	const show = (target: Shown): void => {
		if (next.current.kind === 'none' && target.kind !== 'none') {
			const focused = typeof document === 'undefined' ? null : document.activeElement;
			returnFocus.current =
				focused instanceof HTMLElement && focused !== document.body ? focused : null;
			landed.current = false;
		}
		next.current = target;
		setShown(target);
	};
	const finalFocus = useMemoizedFn((): HTMLElement | false | null => {
		// The other dialog opens in its place and takes the keyboard itself.
		if (next.current.kind !== 'none') return false;
		const captured = returnFocus.current;
		returnFocus.current = null;
		if (landed.current) return false;
		return captured?.isConnected === true
			? captured
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});
	const openBoard = useMemoizedFn(() => show(BOARD));
	const closeBoard = useMemoizedFn(() => show(NONE));
	const showBoardSession = useMemoizedFn(() => {
		// The terminal the jump showed takes the keyboard once the board has gone.
		const handover = beginTerminalHandover();
		show(NONE);
		landed.current = true;
		handover.complete();
	});
	const openStart = useMemoizedFn((places: readonly WorkbenchAgentStartPlace[]) =>
		show({ fromBoard: next.current.kind === 'board', kind: 'start', places }),
	);
	const closeStart = useMemoizedFn(() => {
		const current = next.current;
		show(current.kind === 'start' && current.fromBoard ? BOARD : NONE);
	});
	const endStart = useMemoizedFn(() => show(NONE));

	return {
		board:
			shown.kind === 'board' && agentBoard !== undefined
				? { board: agentBoard, finalFocus }
				: undefined,
		closeBoard,
		closeStart,
		endStart,
		openBoard,
		openStart,
		showBoardSession,
		start:
			shown.kind === 'start' && agentCatalog !== undefined
				? { catalog: agentCatalog, finalFocus, places: shown.places }
				: undefined,
		startOffered,
	};
}
