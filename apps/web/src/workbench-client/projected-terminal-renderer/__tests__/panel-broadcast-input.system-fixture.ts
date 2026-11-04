import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import '@orchbee/ui/styles.css';
import {
	Workbench,
	type WorkbenchAuthority,
	type WorkbenchTerminalBroadcastInput,
	type WorkbenchTerminalSurfaceProps,
} from '../../../workbench';
import { createElement, useEffect, useMemo } from 'react';
import { createRoot } from 'react-dom/client';

import {
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';

const witness = document.createElement('main');
witness.id = 'panel-broadcast-input-witness';
witness.style.height = '500px';
witness.style.width = '1000px';
document.body.replaceChildren(witness);
document.body.style.margin = '0';

const actionRuntime = new ActionRuntime();
const deliveries = new Map<number, string[]>();
const mountedSinks = new Set<number>();
const targetPositions = new Map<string, number>();

/** Three running Panels side by side, as the Server publishes them after two splits to the right. */
const projectWorkspaceRuntime = createSnapshotRuntime(
	oneWorkspaceSnapshot(1, [
		snapshotTab(
			1,
			snapshotSplit(
				1,
				'horizontal',
				snapshotPanel(1),
				snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
			),
		),
	]),
);

/** Broadcast input changes nothing the Server decides, so the witness refuses every shared change. */
function refuse(): Promise<{ readonly code: string; readonly kind: 'blocked' }> {
	return Promise.resolve({ code: 'broadcast-witness-read-only', kind: 'blocked' });
}

const authority: WorkbenchAuthority = {
	change: refuse,
	close: refuse,
	create: refuse,
	createPanelTerminal: refuse,
	createTab: refuse,
	minimumSplitPercent: 10,
	stoppedTabClose: () => null,
	watchClose: () => () => undefined,
};

const panelWorkspaceStore = projectWorkspaceRuntime.getActiveTerminalContext().panelWorkspaceStore;
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
			authority,
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
