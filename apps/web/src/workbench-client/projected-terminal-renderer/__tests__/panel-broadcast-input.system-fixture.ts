import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import '@orchbee/ui/styles.css';
import {
	Workbench,
	WorkbenchProjectWorkspaceRuntime,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type WorkbenchTerminalBroadcastInput,
	type WorkbenchTerminalPresentationCommand,
	type WorkbenchTerminalSurfaceProps,
} from '@orchbee/workbench';
import { createElement, useEffect, useMemo } from 'react';
import { createRoot } from 'react-dom/client';

const witness = document.createElement('main');
witness.id = 'panel-broadcast-input-witness';
witness.style.height = '500px';
witness.style.width = '1000px';
document.body.replaceChildren(witness);
document.body.style.margin = '0';

const actionRuntime = new ActionRuntime();
const deliveries = new Map<number, string[]>();
const mountedSinks = new Set<number>();
const pendingCommands: WorkbenchTerminalPresentationCommand[] = [];
const targetPositions = new Map<string, number>();
let operationSequence = 0;
let presentationSequence = 0;
let removalSequence = 0;

const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
	standaloneCwd: '/workspace/orchbee',
	standaloneTerminalTitle: 'Terminal 1',
	terminalPresentation: {
		createOperationId: () =>
			terminalPresentationOperationId(`broadcast-witness-operation-${++operationSequence}`),
		createPresentationId: () =>
			terminalPresentationId(`broadcast-witness-terminal-${++presentationSequence}`),
		createRemovalId: () =>
			terminalPresentationRemovalId(`broadcast-witness-removal-${++removalSequence}`),
		dispatchCommands: (commands) => pendingCommands.push(...commands),
		initialPresentationId: terminalPresentationId('broadcast-witness-terminal-initial'),
		kind: 'managed',
	},
});

settleProvisioning();
const panelWorkspaceStore = projectWorkspaceRuntime.getActiveTerminalContext().panelWorkspaceStore;
const tabId = panelWorkspaceStore.getActiveTabId();
if (tabId === null) throw new Error('Broadcast witness requires an active terminal Tab.');
for (let index = 0; index < 2; index += 1) {
	const tab = panelWorkspaceStore.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
	if (tab === undefined) throw new Error('Broadcast witness lost its terminal Tab.');
	panelWorkspaceStore.transition({
		direction: 'right',
		newTerminalTitle: `Terminal ${index + 2}`,
		panelId: tab.focusedPanelId,
		presentation: panelWorkspaceStore.createTerminalPresentation(),
		tabId,
		type: 'panel-split',
	});
	settleProvisioning();
}
const broadcastTab = panelWorkspaceStore.getSnapshot().tabs[0];
if (broadcastTab === undefined)
	throw new Error('Broadcast witness did not retain its terminal Tab.');
for (const [index, panel] of collectLeaves(broadcastTab.root).entries()) {
	targetPositions.set(displayedPresentationId(panel), index + 1);
}

createRoot(witness).render(
	createElement(
		OrchbeeI18nProvider,
		{ locale: 'en-US' },
		createElement(Workbench, {
			actionRuntime,
			onSettings: () => ({ kind: 'unchanged' as const }),
			projectWorkspaceRuntime,
			shortcutPlatform: 'apple',
			terminalSurface: BroadcastWitnessTerminalSurface,
		}),
	),
);

function BroadcastWitnessTerminalSurface({
	bindBroadcastInputSink,
	presentation,
}: WorkbenchTerminalSurfaceProps) {
	const position = targetPositions.get(presentation.presentationId);
	if (position === undefined) throw new Error('Broadcast witness found an unknown Presentation.');
	const sink = useMemo(
		() => ({
			getReadiness: () => ({
				kind: 'ready' as const,
				revision: `${presentation.presentationId}:ready`,
			}),
			send: async (input: WorkbenchTerminalBroadcastInput) => {
				const records = deliveries.get(position) ?? [];
				records.push(inputLabel(input));
				deliveries.set(position, records);
				publishDeliveries();
				if (input.kind === 'text' && input.text === 'first') {
					await new Promise((resolve) => globalThis.setTimeout(resolve, 80));
				}
				return input.kind === 'text' && input.text === 'partial' && position === 2
					? { kind: 'failed' as const }
					: { kind: 'sent' as const };
			},
			subscribe: () => ({ dispose: () => undefined }),
		}),
		[position, presentation.presentationId],
	);
	useEffect(() => {
		const registration = bindBroadcastInputSink(sink);
		mountedSinks.add(position);
		if (mountedSinks.size === 3) witness.dataset['witnessReady'] = 'true';
		return () => {
			mountedSinks.delete(position);
			registration.dispose();
		};
	}, [bindBroadcastInputSink, position, sink]);
	return createElement(
		'div',
		{ 'data-broadcast-witness-terminal': position },
		`Terminal ${position}`,
	);
}

function settleProvisioning(): void {
	for (const command of pendingCommands.splice(0)) {
		if (command.type !== 'terminal-provision-requested') continue;
		projectWorkspaceRuntime.applyTerminalPresentationEvent({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-provision-completed',
		});
	}
}

function inputLabel(input: WorkbenchTerminalBroadcastInput): string {
	return input.kind === 'text' ? `text:${input.text}` : `key:${input.key}`;
}

function publishDeliveries(): void {
	witness.dataset['witnessDeliveries'] = [1, 2, 3]
		.map((position) => `${position}:${(deliveries.get(position) ?? []).join('>')}`)
		.join('|');
}

interface WitnessPanel {
	readonly activeChildPresentationId: string | null;
	readonly childPresentations: readonly { readonly presentationId: string }[];
	readonly kind: 'panel';
	readonly primaryPresentation: { readonly presentationId: string };
}

type WitnessNode =
	| WitnessPanel
	| {
			readonly children: readonly { readonly node: WitnessNode }[];
			readonly kind: 'split';
	  };

function collectLeaves(node: WitnessNode): WitnessPanel[] {
	return node.kind === 'panel'
		? [node]
		: node.children.flatMap((child) => collectLeaves(child.node));
}

function displayedPresentationId(panel: WitnessPanel): string {
	if (panel.activeChildPresentationId === null) return panel.primaryPresentation.presentationId;
	const child = panel.childPresentations.find(
		(candidate) => candidate.presentationId === panel.activeChildPresentationId,
	);
	if (child === undefined)
		throw new Error('Broadcast witness found an invalid child Presentation.');
	return child.presentationId;
}
