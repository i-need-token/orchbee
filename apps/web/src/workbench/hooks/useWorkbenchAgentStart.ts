import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentCatalogSource,
	WorkbenchAgentStartPlace,
} from '../types/workbench-agent-start';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';

interface UseWorkbenchAgentStartOptions {
	/** Absent where the authority publishes no Agent catalog; the dialog then never opens. */
	readonly agentCatalog: WorkbenchAgentCatalogSource | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/** What the Start Agent dialog shows while it is open. */
export interface WorkbenchAgentStartDialogModel {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly finalFocus: () => HTMLElement | null;
	/** The places a start can use now, in the dialog's order; the first is the default. */
	readonly places: readonly WorkbenchAgentStartPlace[];
}

interface WorkbenchAgentStart {
	/** Closes the dialog; a start closes it on the same frame it runs the start. */
	readonly close: () => void;
	/** Present while the dialog is open. */
	readonly dialog: WorkbenchAgentStartDialogModel | undefined;
	/** Opens the dialog with the places `workbench.agent.start` found available. */
	readonly open: (places: readonly WorkbenchAgentStartPlace[]) => void;
}

/**
 * The owner of the Start Agent dialog: it opens from `workbench.agent.start` with the places that
 * Action found available, and gives focus back where it was when the dialog closes. A start's own
 * focus handover then gives the new terminal the keyboard.
 */
export function useWorkbenchAgentStart({
	agentCatalog,
	projectWorkspaceRuntime,
}: UseWorkbenchAgentStartOptions): WorkbenchAgentStart {
	const returnFocus = React.useRef<HTMLElement | null>(null);
	const [places, setPlaces] = React.useState<readonly WorkbenchAgentStartPlace[] | null>(null);
	const open = useMemoizedFn((offered: readonly WorkbenchAgentStartPlace[]) => {
		const focused = typeof document === 'undefined' ? null : document.activeElement;
		returnFocus.current =
			focused instanceof HTMLElement && focused !== document.body ? focused : null;
		setPlaces(offered);
	});
	const close = useMemoizedFn(() => setPlaces(null));
	const finalFocus = useMemoizedFn((): HTMLElement | null => {
		const captured = returnFocus.current;
		returnFocus.current = null;
		return captured?.isConnected === true
			? captured
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});

	return {
		close,
		dialog:
			places === null || agentCatalog === undefined
				? undefined
				: { catalog: agentCatalog, finalFocus, places },
		open,
	};
}
