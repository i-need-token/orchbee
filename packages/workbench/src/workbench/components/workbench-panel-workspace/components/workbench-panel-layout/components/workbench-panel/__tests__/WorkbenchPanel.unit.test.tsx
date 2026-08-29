import { ActionRuntime } from '@orchbee/actions';
import { AgentProviderIdSchema, type AgentLocationPresentation } from '@orchbee/agent';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement, createRef } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '../../../../../../../../terminal-presentation';
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
	readonly agentPresentation?: AgentLocationPresentation | undefined;
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
		canCreateTerminalTabInCurrentDirectory: () => true,
		canResizePanel: () => false,
		cancelClose: () => undefined,
		closeActiveTerminal: () => undefined,
		confirmClose: () => undefined,
		createChildTerminal: () => undefined,
		createTerminalTab: () => undefined,
		createTerminalTabInCurrentDirectory: () => undefined,
		displayedRoot: panel,
		equalizePanels: () => undefined,
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
			closeAnyway: 'Close anyway',
			closeRunningDescription: () => 'Running',
			closeRunningTitle: 'Close running terminal?',
			equalize: 'Equalize Panels',
			maximize: 'Maximize Panel',
			minimize: 'Minimize Panel',
			more: 'Panel options',
			newChildTerminal: 'New child terminal',
			newTab: 'New tab',
			newTabCurrentDirectory: 'New tab in current directory',
			returnToPrimary: 'Return to primary terminal',
			resize: 'Resize Panel…',
			resizeDescription: 'Use W, N, T, or S to resize.',
			resizeDone: 'Done',
			resizeNarrower: 'Narrower',
			resizeReset: 'Reset all splits',
			resizeShorter: 'Shorter',
			resizeTaller: 'Taller',
			resizeTitle: 'Resize Panel',
			resizeUnavailable: 'Unavailable',
			resizeWider: 'Wider',
			split: 'Split panel',
			splitAbove: 'Split above',
			splitBelow: 'Split below',
			splitLeft: 'Split left',
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
		},
		panelVisualSelection: null,
		panelVisualSelectionLabel: undefined,
		pendingClose: null,
		pendingResize: null,
		requestClose: () => undefined,
		dismissBroadcastInput: () => undefined,
		resizeAdjacentPanels: () => undefined,
		resizePanel: () => false,
		showPrimaryTerminal: () => undefined,
		shortcutLabelForAction: () => undefined,
		splitPanel: () => null,
		startPanelDrag: () => undefined,
		startBroadcastInput: () => undefined,
		startPanelResize: () => undefined,
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
					agentLocations:
						options.agentPresentation === undefined
							? undefined
							: new Map([['terminal-1', options.agentPresentation]]),
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

	it('shows a localized, non-color-only Agent status without replacing the terminal title', () => {
		const markup = renderPanel(createPanel(null), 1, {
			agentPresentation: {
				activeCount: 1,
				attentionCount: 2,
				freshness: 'stale',
				location: {
					generation: 'binding-1',
					navigable: true,
					presentationId: 'terminal-1',
					visibility: 'visible',
				},
				providerIds: [AgentProviderIdSchema.parse('claude-code')],
				sessionCount: 1,
				status: { count: 2, kind: 'needs-attention' },
			},
		});

		expect(markup).toContain('data-slot="workbench-panel-header"');
		expect(markup).toContain('data-slot="workbench-agent-status"');
		expect(markup).toContain('data-icon="agent"');
		expect(markup).toContain('Claude Code');
		expect(markup).toContain('Needs attention (2) · Status stale');
		expect(markup).toContain('data-slot="workbench-panel-header-title">Terminal 1</div>');
	});

	it('shows return and close actions but no panel menu for a singleton child terminal', () => {
		const markup = renderPanel(createPanel(terminalPresentationId('terminal-2')), 1);

		expect(markup).toContain('data-terminal-role="child"');
		expect(markup).toContain('data-slot="workbench-panel-header"');
		expect(markup).toContain('data-slot="workbench-child-terminal-strip"');
		expect(markup).not.toContain('data-slot="context-menu-trigger"');
		expect(markup).toContain('data-icon="arrow-left"');
		expect(markup).toContain('data-icon="close"');
		expect(markup).not.toContain('data-drag-enabled');
		expect(markup).not.toContain('data-icon="ellipsis"');
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
