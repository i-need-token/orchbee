import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement, createRef } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import type { PanelLayoutNode, PanelLeaf } from '../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../types/panel-workspace-view';
import type { TerminalTabPresentationLayer } from '../../../../../types/terminal-tab-presentation-layer';
import type { WorkbenchTerminalSurfaceProps } from '../../../../../../../types/workbench-terminal-surface';
import { WorkbenchPanel } from '../WorkbenchPanel';

const primaryPresentation = {
	closeActivity: 'unknown',
	currentCwd: null,
	launchCwd: null,
	lifecycle: { kind: 'preview' },
	naturalTitle: null,
	presentationId: terminalPresentationId('terminal-1'),
	title: 'Terminal 1',
} as const;
const childTerminal = {
	closeActivity: 'unknown',
	currentCwd: null,
	launchCwd: null,
	lifecycle: { kind: 'preview' },
	naturalTitle: null,
	presentationId: terminalPresentationId('terminal-2'),
	title: 'Terminal 2',
} as const;
const otherPanel = {
	activeChildPresentationId: null,
	childPresentations: [],
	id: 'panel-2',
	kind: 'panel',
	primaryPresentation: {
		closeActivity: 'unknown',
		currentCwd: null,
		launchCwd: null,
		lifecycle: { kind: 'preview' },
		naturalTitle: null,
		presentationId: terminalPresentationId('terminal-3'),
		title: 'Terminal 3',
	},
} as const;
const layer: TerminalTabPresentationLayer = {
	epoch: 0,
	mode: 'active',
	reportPresentationReady: () => undefined,
};

interface RenderPanelOptions {
	readonly dragging?: boolean;
	readonly focused?: boolean;
	readonly layoutSettled?: boolean;
}

function TestTerminalSurface({ autoFocus }: WorkbenchTerminalSurfaceProps) {
	return createElement('div', { 'data-terminal-auto-focus': String(autoFocus) });
}

function createPanel(activeChildPresentationId: TerminalPresentationId | null): PanelLeaf {
	return {
		activeChildPresentationId,
		childPresentations: activeChildPresentationId === null ? [] : [childTerminal],
		id: 'panel-1',
		kind: 'panel',
		primaryPresentation,
	};
}

function renderPanel(
	panel: PanelLeaf,
	panelCount: number,
	options: RenderPanelOptions = {},
): string {
	const root: PanelLayoutNode =
		panelCount === 1
			? panel
			: {
					children: [
						{ id: panel.id, node: panel, weight: 50 },
						{ id: otherPanel.id, node: otherPanel, weight: 50 },
					],
					id: 'split-1',
					kind: 'split',
					orientation: 'horizontal',
				};
	const tab = {
		focusedPanelId: options.focused === false ? otherPanel.id : panel.id,
		maximizedPanelId: null,
		panelFocusHistory: [options.focused === false ? otherPanel.id : panel.id],
		root,
		tabId: 'tab-a',
	} as const;
	const actionRuntime = new ActionRuntime();
	const view: WorkbenchPanelWorkspaceView = {
		actionRuntime,
		actionScope: actionRuntime.createScope('test.workbench'),
		admitTerminalBellEffect: () => true,
		activateChildTerminal: () => undefined,
		activeTab: tab,
		bindBroadcastInputSink: () => ({ dispose: () => undefined }),
		broadcastInput: { kind: 'idle' },
		cancelPanelVisualSelection: () => undefined,
		cancelPanelResize: () => undefined,
		canStartBroadcastInput: () => false,
		canResizePanel: () => false,
		cancelClose: () => undefined,
		closeActiveTerminal: () => undefined,
		confirmClose: () => undefined,
		createChildTerminal: () => undefined,
		createTerminalTab: () => undefined,
		displayedRoot: panel,
		enqueueBroadcastInput: () => undefined,
		endBroadcastInput: () => undefined,
		focusPanel: () => undefined,
		handlePanelVisualSelectionKeyDown: () => undefined,
		handlePanelVisualSelectionPointerDown: () => undefined,
		maximizedPanelId: null,
		messages: {
			broadcastDeliveryFailed: 'Broadcast input delivery failed.',
			broadcastDismiss: 'Dismiss',
			broadcastEnd: 'End',
			broadcastHideInput: 'Hide input',
			broadcastInputLabel: 'Broadcast input',
			broadcastInputPlaceholder: 'Type to all Panels',
			broadcastPartialDelivery: (delivered, total) => `${delivered} of ${total} delivered`,
			broadcastShowInput: 'Show input',
			broadcastStart: 'Broadcast input',
			broadcastTargetCount: (count) => `${count} targets`,
			broadcastTargetInvalidated: 'Broadcast targets changed.',
			cancel: 'Cancel',
			childPresentationsLabel: 'Child terminals',
			close: 'Close',
			closeChildTitle: 'Close running terminal?',
			closeTab: (title) => `Tab: ${title}`,
			closeTitle: 'Close Panel?',
			closeWarning: () => 'Ends 1 terminal.',
			maximize: 'Maximize',
			more: 'Panel options',
			newChildTerminal: 'New child terminal',
			restore: 'Restore',
			returnToPrimary: 'Return to primary terminal',
			resizeDescription: 'Use W, N, T, or S to resize.',
			resizeDone: 'Done',
			resizeNarrower: 'Narrower',
			resizeReset: 'Reset all splits',
			resizeShorter: 'Shorter',
			resizeTaller: 'Taller',
			resizeTitle: 'Resize Panel',
			resizeUnavailable: 'Unavailable',
			resizeWider: 'Wider',
			splitBelow: 'Split below',
			splitRight: 'Split right',
			visualSelectionFocusLabel: 'Choose a panel to focus.',
			visualSelectionFocusTarget: (label) => `Focus panel ${label}`,
			visualSelectionSwapLabel: 'Choose a panel to swap.',
			visualSelectionSwapTarget: (label) => `Swap with panel ${label}`,
		},
		panelCount,
		panelDrag: {
			draggingPanelId: options.dragging === true ? panel.id : null,
			dropTarget: null,
			ghost: null,
			previewRoot: options.dragging === true ? panel : null,
			sourceRevision: null,
		},
		panelVisualSelection: null,
		panelVisualSelectionLabel: undefined,
		pendingClose: null,
		pendingResize: null,
		releasePanelDragHold: () => undefined,
		requestClose: () => undefined,
		dismissBroadcastInput: () => undefined,
		resizePanel: () => Promise.resolve(false),
		sharedSplitResize: { commit: () => Promise.resolve(), minimumChildPercent: 10 },
		showPrimaryTerminal: () => undefined,
		shortcutLabelForAction: () => undefined,
		splitPanel: () => null,
		startPanelDrag: () => undefined,
		startBroadcastInput: () => undefined,
		state: {
			nextPanelSequence: 2,
			nextPanelSlotSequence: 1,
			nextSplitSequence: 1,
			nextTerminalSequence: 3,
			pendingRemovals: [],
			tabs: [tab],
		},
		togglePanelMaximization: () => undefined,
		toggleBroadcastInputVisibility: () => undefined,
		workspaceReference: createRef<HTMLDivElement>(),
	};
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(
				TooltipProvider,
				null,
				createElement(WorkbenchPanel, {
					layer,
					layoutSettled: options.layoutSettled ?? true,
					panel,
					terminalAutoFocusAllowed: true,
					terminalFocusRequest: null,
					terminalSurface: TestTerminalSurface,
					view,
				}),
			),
		),
	);
}

describe('WorkbenchPanel', () => {
	it('gives a singleton primary terminal the full panel without chrome', () => {
		const markup = renderPanel(createPanel(null), 1);

		expect(markup).toContain('data-terminal-role="primary"');
		expect(markup).toContain('data-terminal-presentation-id="terminal-1"');
		expect(markup).not.toContain('data-slot="workbench-panel-header"');
		expect(markup).not.toContain('data-slot="workbench-child-terminal-strip"');
		expect(markup).not.toContain('data-slot="workbench-panel-focus-scrim"');
	});

	it('gives a singleton child terminal return, panel options and close', () => {
		const markup = renderPanel(createPanel(terminalPresentationId('terminal-2')), 1);

		expect(markup).toContain('data-terminal-role="child"');
		expect(markup).toContain('data-slot="workbench-panel-header"');
		expect(markup).toContain('data-slot="workbench-child-terminal-strip"');
		expect(markup).toContain('data-slot="context-menu-trigger"');
		expect(markup).toContain('data-icon="arrow-left"');
		expect(markup).toContain('data-icon="close"');
		expect(markup).not.toContain('data-drag-enabled');
		expect(markup).toContain('data-icon="ellipsis"');
	});

	it('shows panel options for a primary terminal in a split layout', () => {
		const markup = renderPanel(createPanel(null), 2);

		expect(markup).toContain('data-slot="workbench-panel-header"');
		expect(markup).toContain('data-slot="context-menu-trigger"');
		expect(markup).toContain('data-drag-enabled="true"');
		expect(markup).toContain('data-icon="ellipsis"');
		expect(markup).not.toContain('data-icon="more-vertical"');
		expect(markup).not.toContain('data-icon="arrow-left"');
		expect(markup).not.toContain('data-icon="search"');
	});

	it('projects the focused terminal natural title into the panel header', () => {
		const panel = createPanel(null);
		const markup = renderPanel(
			{
				...panel,
				primaryPresentation: { ...panel.primaryPresentation, naturalTitle: 'vim' },
			},
			2,
		);

		expect(markup).toContain('data-slot="workbench-panel-header-title">vim</div>');
	});

	it('masks only a non-focused panel when the workspace is split', () => {
		const focusedMarkup = renderPanel(createPanel(null), 2);
		const inactiveMarkup = renderPanel(createPanel(null), 2, { focused: false });

		expect(focusedMarkup).toContain('data-focused="true"');
		expect(focusedMarkup).not.toContain('data-slot="workbench-panel-focus-scrim"');
		expect(inactiveMarkup).toContain('data-inactive="true"');
		expect(inactiveMarkup).toContain('data-slot="workbench-panel-focus-scrim"');
	});

	it('projects terminal auto focus only to the focused panel', () => {
		const focusedMarkup = renderPanel(createPanel(null), 2);
		const inactiveMarkup = renderPanel(createPanel(null), 2, { focused: false });
		const unsettledMarkup = renderPanel(createPanel(null), 2, { layoutSettled: false });

		expect(focusedMarkup).toContain('data-terminal-auto-focus="true"');
		expect(inactiveMarkup).toContain('data-terminal-auto-focus="false"');
		expect(unsettledMarkup).toContain('data-terminal-auto-focus="false"');
	});

	it('renders only an empty framed placeholder at the projected drop position', () => {
		const markup = renderPanel(createPanel(null), 2, { dragging: true });

		expect(markup).toContain('data-slot="workbench-panel-drop-placeholder"');
		expect(markup).not.toContain('data-slot="workbench-panel-header"');
		expect(markup).not.toContain('data-slot="workbench-terminal-viewport"');
	});
});
