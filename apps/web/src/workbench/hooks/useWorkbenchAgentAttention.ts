import * as React from 'react';

import {
	WorkbenchAgentAttention,
	type WorkbenchAgentAttentionSnapshot,
} from '../model/WorkbenchAgentAttention';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchTerminalContext } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';
import { subscribeToNothing } from '../utils/subscribeToNothing';
import { hasWorkbenchExclusiveFocusSurface } from '../utils/workbench-focus-dom';

/** Receives the exact states a visible window painted, focused or not. */
type WorkbenchAgentPaintWitness = (
	observation: WorkbenchAgentSnapshot,
	presentationIds: ReadonlySet<string>,
) => void;

/** Only the active Workspace's terminals can be seen, so none are while no Workspace is open. */
export function useWorkbenchAgentAttention(
	source: WorkbenchAgentSource | undefined,
	terminalContext: WorkbenchTerminalContext | null,
	root: React.RefObject<HTMLElement | null>,
	painted: WorkbenchAgentPaintWitness,
): WorkbenchAgentAttentionSnapshot & { readonly terminalEdited: (presentationId: string) => void } {
	const attention = React.useMemo(() => new WorkbenchAgentAttention(source), [source]);
	const snapshot = React.useSyncExternalStore(
		attention.subscribe,
		attention.getSnapshot,
		attention.getSnapshot,
	);
	const panelWorkspaceStore = terminalContext?.panelWorkspaceStore;
	const tabModel = terminalContext?.tabModel;
	const readPanels = () => panelWorkspaceStore?.getSnapshot() ?? null;
	const readTabs = () => tabModel?.getSnapshot() ?? null;
	const panels = React.useSyncExternalStore(
		panelWorkspaceStore?.subscribe ?? subscribeToNothing,
		readPanels,
		readPanels,
	);
	const tabs = React.useSyncExternalStore(
		tabModel?.subscribe ?? subscribeToNothing,
		readTabs,
		readTabs,
	);
	React.useEffect(() => {
		const element = root.current;
		if (element === null) return;
		const visiblePresentations = new Set<string>();
		const active = panels?.tabs.find((tab) => tab.tabId === tabs?.activeTabId);
		if (active !== undefined) {
			for (const panel of collectPanelLeaves(active.root)) {
				if (active.maximizedPanelId === null || active.maximizedPanelId === panel.id) {
					visiblePresentations.add(
						panel.activeChildPresentationId ?? panel.primaryPresentation.presentationId,
					);
				}
			}
		}
		let frame = 0;
		const schedule = () => {
			cancelAnimationFrame(frame);
			// Allow a visible browser frame before acknowledging the exact observed version.
			frame = requestAnimationFrame(() => {
				frame = requestAnimationFrame(() => {
					if (
						panels !== (panelWorkspaceStore?.getSnapshot() ?? null) ||
						tabs !== (tabModel?.getSnapshot() ?? null)
					)
						return;
					if (document.visibilityState !== 'visible') return;
					// Selection can lead the visual handoff. Only a ready, active terminal
					// layer from this Workbench can witness a paint.
					const displayed = new Set<string>();
					for (const viewport of element.querySelectorAll(
						'[data-presentation-mode="active"] [data-terminal-display-ready="true"]',
					)) {
						const id = viewport.getAttribute('data-terminal-presentation-id');
						if (id !== null && visiblePresentations.has(id)) displayed.add(id);
					}
					painted(snapshot.observation, displayed);
					attention.acknowledge(
						snapshot,
						displayed,
						document.hasFocus() && !hasWorkbenchExclusiveFocusSurface(document),
					);
				});
			});
		};
		const observer = new MutationObserver(schedule);
		observer.observe(element, {
			attributeFilter: ['data-presentation-mode', 'data-terminal-display-ready'],
			attributes: true,
			subtree: true,
		});
		schedule();
		window.addEventListener('focus', schedule);
		document.addEventListener('focusin', schedule);
		document.addEventListener('visibilitychange', schedule);
		return () => {
			observer.disconnect();
			cancelAnimationFrame(frame);
			window.removeEventListener('focus', schedule);
			document.removeEventListener('focusin', schedule);
			document.removeEventListener('visibilitychange', schedule);
		};
	}, [attention, snapshot, painted, panels, tabs, panelWorkspaceStore, tabModel, root]);
	return { ...snapshot, terminalEdited: attention.terminalEdited };
}
