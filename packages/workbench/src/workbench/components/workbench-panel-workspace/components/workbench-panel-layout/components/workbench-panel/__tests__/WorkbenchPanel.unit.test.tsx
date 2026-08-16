import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '../../../../../../../../terminal-presentation';
import type { PanelLayoutNode, PanelLeaf } from '../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../types/panel-workspace-view';
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

interface RenderPanelOptions {
	readonly dragging?: boolean;
	readonly focused?: boolean;
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
		root,
		tabId: 'tab-a',
	} as const;
	const view: WorkbenchPanelWorkspaceView = {
		activateChildTerminal: () => undefined,
		activeTab: tab,
		cancelClose: () => undefined,
		confirmClose: () => undefined,
		createChildTerminal: () => undefined,
		displayedRoot: panel,
		focusPanel: () => undefined,
		maximizedPanelId: null,
		messages: {
			cancel: 'Cancel',
			childPresentationsLabel: 'Child terminals',
			close: 'Close',
			closeAnyway: 'Close anyway',
			closeRunningDescription: () => 'Running',
			closeRunningTitle: 'Close running terminal?',
			maximize: 'Maximize Panel',
			minimize: 'Minimize Panel',
			more: 'Panel options',
			newChildTerminal: 'New child terminal',
			returnToPrimary: 'Return to primary terminal',
			splitAbove: 'Split above',
			splitBelow: 'Split below',
			splitLeft: 'Split left',
			splitRight: 'Split right',
		},
		panelCount,
		panelDrag: {
			draggingPanelId: options.dragging === true ? panel.id : null,
			dropTarget: null,
			ghost: null,
			previewRoot: options.dragging === true ? panel : null,
		},
		pendingClose: null,
		requestClose: () => undefined,
		resizeAdjacentPanels: () => undefined,
		showPrimaryTerminal: () => undefined,
		splitPanel: () => null,
		startPanelDrag: () => undefined,
		state: {
			nextPanelSequence: 2,
			nextPanelSlotSequence: 1,
			nextSplitSequence: 1,
			nextTerminalSequence: 3,
			pendingRemovals: [],
			tabs: [tab],
		},
		togglePanelMaximization: () => undefined,
	};
	return renderToStaticMarkup(
		createElement(TooltipProvider, null, createElement(WorkbenchPanel, { panel, view })),
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

	it('renders only an empty framed placeholder at the projected drop position', () => {
		const markup = renderPanel(createPanel(null), 2, { dragging: true });

		expect(markup).toContain('data-slot="workbench-panel-drop-placeholder"');
		expect(markup).not.toContain('data-slot="workbench-panel-header"');
		expect(markup).not.toContain('data-slot="workbench-terminal-viewport"');
	});
});
