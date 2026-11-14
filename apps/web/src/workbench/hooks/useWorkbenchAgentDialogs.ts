import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentCatalogSource,
	WorkbenchAgentStartPlace,
} from '../types/workbench-agent-start';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import { subscribeToNothing } from '../utils/subscribeToNothing';

interface UseWorkbenchAgentDialogsOptions {
	/** Absent where the authority publishes no Agent catalog; Start Agent then never opens. */
	readonly agentCatalog: WorkbenchAgentCatalogSource | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/** What Start Agent shows while it is open. */
export interface WorkbenchAgentStartDialogModel {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly finalFocus: () => HTMLElement | false | null;
	/** The places a start can use now, in the dialog's order; the first is the default. */
	readonly places: readonly WorkbenchAgentStartPlace[];
}

interface WorkbenchAgentDialogs {
	/** Closes Start Agent, returning the keyboard to where it was. */
	readonly closeStart: () => void;
	/**
	 * A start ran: Start Agent closes on this frame. `handedOff` says a dialog the start opened, the
	 * New Worktree form, takes the keyboard in its place.
	 */
	readonly endStart: (handedOff: boolean) => void;
	/** Opens Start Agent with the places `workbench.agent.start` found available. */
	readonly openStart: (places: readonly WorkbenchAgentStartPlace[]) => void;
	/** Present while Start Agent is open. */
	readonly start: WorkbenchAgentStartDialogModel | undefined;
	/** Whether the Session can start an Agent now, which offers Start Agent's entry points. */
	readonly startOffered: boolean;
}

type Shown =
	| { readonly kind: 'none' }
	| { readonly kind: 'start'; readonly places: readonly WorkbenchAgentStartPlace[] };

const NONE: Shown = { kind: 'none' };

/**
 * The one owner of Start Agent. Focus returns, when it closes, to where it was before it opened; a
 * start that hands the keyboard to another dialog leaves focus to that dialog instead.
 */
export function useWorkbenchAgentDialogs({
	agentCatalog,
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
		if (next.current.kind !== 'none') return false;
		const captured = returnFocus.current;
		returnFocus.current = null;
		if (landed.current) return false;
		return captured?.isConnected === true
			? captured
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});
	const openStart = useMemoizedFn((places: readonly WorkbenchAgentStartPlace[]) =>
		show({ kind: 'start', places }),
	);
	const closeStart = useMemoizedFn(() => show(NONE));
	const endStart = useMemoizedFn((handedOff: boolean) => {
		show(NONE);
		if (handedOff) landed.current = true;
	});

	return {
		closeStart,
		endStart,
		openStart,
		start:
			shown.kind === 'start' && agentCatalog !== undefined
				? { catalog: agentCatalog, finalFocus, places: shown.places }
				: undefined,
		startOffered,
	};
}
