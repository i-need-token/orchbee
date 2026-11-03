import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { WorkbenchActionContextMenuContent } from '../../../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../../../../../types/workbench-action-menu';
import {
	getTerminalDisplayTitle,
	type TerminalPresentation,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import type { TerminalTabPresentationLayer } from '../../../../../../types/terminal-tab-presentation-layer';
import type {
	WorkbenchTerminalSurface,
	WorkbenchTerminalSurfaceProps,
} from '../../../../../../../../types/workbench-terminal-surface';
import type { WorkbenchTerminalFocusRequest } from '../../../../../../../../model/resolveWorkbenchFocusTarget';
import { buildWorkbenchPanelMenuEntries } from '../../utils/buildWorkbenchPanelMenuEntries';
import { useWorkbenchTerminalContextMenu } from './hooks/useWorkbenchTerminalContextMenu';
import { useWorkbenchTerminalActions } from './hooks/useWorkbenchTerminalActions';
import styles from './WorkbenchTerminalViewport.module.css';

interface WorkbenchTerminalViewportProps {
	readonly layer: TerminalTabPresentationLayer;
	readonly layoutSettled: boolean;
	readonly panelId: string;
	readonly terminal: TerminalPresentation;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly terminalRole: 'child' | 'primary';
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchTerminalViewport({
	layer,
	layoutSettled,
	panelId,
	terminal,
	terminalAutoFocusAllowed,
	terminalFocusRequest,
	terminalRole,
	terminalSurface: TerminalSurface,
	view,
}: WorkbenchTerminalViewportProps): React.ReactNode {
	const [displayReady, setDisplayReady] = React.useState<{
		readonly presentationId: string;
		readonly surface: WorkbenchTerminalSurface;
	} | null>(null);
	const contextMenu = useWorkbenchTerminalContextMenu({
		panelId,
		splitPanel: view.splitPanel,
	});
	const terminalActions = useWorkbenchTerminalActions({
		parentScope: view.actionScope,
		presentationId: terminal.presentationId,
		runtime: view.actionRuntime,
		shortcutLabelForAction: view.shortcutLabelForAction,
	});
	const bindBroadcastInputSink = useMemoizedFn(
		(sink: Parameters<WorkbenchTerminalSurfaceProps['bindBroadcastInputSink']>[0]) =>
			view.bindBroadcastInputSink(terminal.presentationId, sink),
	);
	const panelEntries = buildWorkbenchPanelMenuEntries({
		panelId,
		source: 'context-menu',
		split: contextMenu.split,
		takeControl: terminalActions.takeControl,
		view,
	});
	const entries: readonly WorkbenchActionMenuEntry[] =
		terminalActions.entries.length === 0
			? panelEntries
			: [
					...terminalActions.entries,
					{ id: 'terminal-product-actions', kind: 'separator' },
					...panelEntries,
				];

	return (
		<ContextMenu>
			<ContextMenuTrigger
				aria-label={getTerminalDisplayTitle(terminal)}
				className={styles['viewport']}
				data-panel-id={panelId}
				data-slot='workbench-terminal-viewport'
				data-terminal-presentation-id={terminal.presentationId}
				data-terminal-display-ready={
					displayReady?.presentationId === terminal.presentationId &&
					displayReady.surface === TerminalSurface
						? 'true'
						: undefined
				}
				data-terminal-role={terminalRole}
				onContextMenu={(event) => {
					if (terminalActions.captureContext(event)) {
						contextMenu.captureFinalFocus(event.currentTarget.ownerDocument);
						return;
					}
					event.preventDefault();
					event.stopPropagation();
				}}
				tabIndex={0}
			>
				{TerminalSurface === undefined || (layer.mode === 'incoming' && !layoutSettled) ? null : (
					<TerminalSurface
						actionRuntime={view.actionRuntime}
						actionScope={terminalActions.scope}
						admitBellEffect={view.admitTerminalBellEffect}
						autoFocus={
							terminalAutoFocusAllowed &&
							view.broadcastInput.kind === 'idle' &&
							layoutSettled &&
							layer.mode === 'active' &&
							view.activeTab?.focusedPanelId === panelId
						}
						focusRequest={
							terminalFocusRequest?.presentationId === terminal.presentationId
								? terminalFocusRequest.sequence
								: 0
						}
						bindActionContextProvider={terminalActions.bindActionContextProvider}
						bindBroadcastInputSink={bindBroadcastInputSink}
						closeTerminal={(source) => view.closeActiveTerminal(panelId, source)}
						createTab={view.createTerminalTab}
						onTerminalEdit={() => view.onTerminalEdit?.(terminal.presentationId)}
						onPresentationReady={(readiness) => {
							setDisplayReady((current) =>
								current?.presentationId === terminal.presentationId &&
								current.surface === TerminalSurface
									? current
									: { presentationId: terminal.presentationId, surface: TerminalSurface },
							);
							layer.reportPresentationReady(terminal.presentationId, readiness);
						}}
						presentation={terminal}
						presentationMode={layer.mode}
					/>
				)}
			</ContextMenuTrigger>
			<WorkbenchActionContextMenuContent entries={entries} finalFocus={contextMenu.finalFocus} />
		</ContextMenu>
	);
}
