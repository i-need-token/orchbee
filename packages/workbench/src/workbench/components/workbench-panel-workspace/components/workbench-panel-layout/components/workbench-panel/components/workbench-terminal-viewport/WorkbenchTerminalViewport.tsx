import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { WorkbenchActionContextMenuContent } from '../../../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import { WORKBENCH_ACTION_IDS } from '../../../../../../../../actions/workbench-action-ids';
import type { WorkbenchActionMenuEntry } from '../../../../../../../../types/workbench-action-menu';
import {
	getTerminalDisplayTitle,
	type TerminalPresentation,
	type PanelSplitDirection,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import type { TerminalTabPresentationLayer } from '../../../../../../types/terminal-tab-presentation-layer';
import type {
	WorkbenchTerminalSurface,
	WorkbenchTerminalSurfaceProps,
} from '../../../../../../../../types/workbench-terminal-surface';
import type { WorkbenchTerminalFocusRequest } from '../../../../../../../../model/resolveWorkbenchFocusTarget';
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
	const split = (direction: PanelSplitDirection): void => contextMenu.split(direction);
	const bindBroadcastInputSink = useMemoizedFn(
		(sink: Parameters<WorkbenchTerminalSurfaceProps['bindBroadcastInputSink']>[0]) =>
			view.bindBroadcastInputSink(terminal.presentationId, sink),
	);
	const structuralEntries: readonly WorkbenchActionMenuEntry[] = [
		{
			icon: 'tabs',
			id: 'new-terminal-tab',
			kind: 'command',
			label: view.messages.newTab,
			onSelect: () => view.createTerminalTab('context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.createTerminal),
		},
		{
			disabled: !view.canCreateTerminalTabInCurrentDirectory(panelId),
			icon: 'tabs',
			id: 'new-terminal-tab-current-directory',
			kind: 'command',
			label: view.messages.newTabCurrentDirectory,
			onSelect: () => view.createTerminalTabInCurrentDirectory(panelId, 'context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory),
		},
		{
			icon: 'terminal',
			id: 'new-child-terminal',
			kind: 'command',
			label: view.messages.newChildTerminal,
			onSelect: () => view.createChildTerminal(panelId, 'context-menu'),
		},
		{
			icon: 'panels',
			id: 'resize-panel',
			kind: 'command',
			label: view.messages.resize,
			onSelect: () => view.startPanelResize(panelId, 'context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.resizeActivePanel),
		},
		{
			disabled: !view.canStartBroadcastInput(panelId),
			icon: 'terminal',
			id: 'broadcast-panel-input',
			kind: 'command',
			label: view.messages.broadcastStart,
			onSelect: () => view.startBroadcastInput(panelId, 'context-menu'),
		},
		{
			disabled: view.panelCount < 2 || view.maximizedPanelId !== null,
			icon: 'panels',
			id: 'equalize-panels',
			kind: 'command',
			label: view.messages.equalize,
			onSelect: () => view.equalizePanels(panelId, 'context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.equalizeActivePanels),
		},
		{
			disabled: view.panelCount < 2,
			icon: view.maximizedPanelId === panelId ? 'screen-normal' : 'screen-full',
			id: 'toggle-panel-maximization',
			kind: 'command',
			label: view.maximizedPanelId === panelId ? view.messages.minimize : view.messages.maximize,
			onSelect: () => view.togglePanelMaximization(panelId, 'context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.toggleActivePanelMaximization),
		},
		{ id: 'terminal-split-actions', kind: 'separator' },
		{
			groups: [
				{
					commands: [
						{
							icon: 'panels',
							id: 'split-panel-right',
							kind: 'command',
							label: view.messages.splitRight,
							onSelect: () => split('right'),
							shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.splitActivePanelRight),
						},
						{
							icon: 'panels',
							id: 'split-panel-left',
							kind: 'command',
							label: view.messages.splitLeft,
							onSelect: () => split('left'),
						},
						{
							icon: 'panels',
							id: 'split-panel-below',
							kind: 'command',
							label: view.messages.splitBelow,
							onSelect: () => split('below'),
							shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.splitActivePanelBelow),
						},
						{
							icon: 'panels',
							id: 'split-panel-above',
							kind: 'command',
							label: view.messages.splitAbove,
							onSelect: () => split('above'),
						},
					],
					id: 'split-panel-directions',
				},
			],
			icon: 'panels',
			id: 'split-panel',
			kind: 'submenu',
			label: view.messages.split,
		},
		{ id: 'terminal-close-actions', kind: 'separator' },
		{
			icon: 'close',
			id: 'close-active-terminal',
			kind: 'command',
			label: view.messages.close,
			onSelect: () => view.closeActiveTerminal(panelId, 'context-menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeActiveTerminal),
		},
	];
	const entries: readonly WorkbenchActionMenuEntry[] =
		terminalActions.entries.length === 0
			? structuralEntries
			: [
					...terminalActions.entries,
					{ id: 'terminal-product-actions', kind: 'separator' },
					...structuralEntries,
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
