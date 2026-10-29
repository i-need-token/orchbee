import {
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalPresentationComposition,
} from '@orchbee/workbench';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedPresentation,
	type TerminalPresentationId,
	type TerminalPresentationOperationId,
	type TerminalPresentationRemovalId,
} from '@orchbee/workbench-client-protocol';

import type { DesktopRecoveredTerminalPresentation } from '../terminal-document-control';

export interface DesktopWorkbenchInitializationOptions {
	readonly createOperationId: () => TerminalPresentationOperationId;
	readonly createPresentationId: () => TerminalPresentationId;
	readonly createRemovalId: () => TerminalPresentationRemovalId;
	readonly defaultCwd: string;
	readonly initialPresentationId: TerminalPresentationId;
	readonly recoveredPresentations: readonly DesktopRecoveredTerminalPresentation[];
	readonly terminalTitle: string;
}

export function createDesktopWorkbenchInitializationDocument(
	options: DesktopWorkbenchInitializationOptions,
): WorkbenchSharedDocument {
	const composition: WorkbenchTerminalPresentationComposition = {
		createOperationId: options.createOperationId,
		createPresentationId: options.createPresentationId,
		createRemovalId: options.createRemovalId,
		dispatchCommands: () => undefined,
		initialPresentationId: options.initialPresentationId,
		kind: 'managed',
	};
	const recovered = options.recoveredPresentations.map((presentation) => ({
		cwd: presentation.launchCwd ?? options.defaultCwd,
		presentationId: presentation.presentationId,
		title: options.terminalTitle,
	}));
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: options.defaultCwd,
		standaloneTerminalTitle: options.terminalTitle,
		...(recovered.length === 0 ? {} : { standalonePresentations: recovered }),
		terminalPresentation: composition,
	});
	const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('0'));
	if (recovered.length === 0) return document;
	return createWorkbenchSharedDocument({
		projectWorkspace: document.projectWorkspace,
		revision: document.revision,
		terminalContexts: document.terminalContexts.map((context) => ({
			...context,
			panelWorkspace: {
				...context.panelWorkspace,
				tabs: context.panelWorkspace.tabs.map((tab) => ({
					...tab,
					root: retainPresentations(tab.root),
				})),
			},
		})),
	});
}

function retainPresentations(node: WorkbenchSharedPanelNode): WorkbenchSharedPanelNode {
	if (node.kind === 'split') {
		return {
			...node,
			children: node.children.map((child) => ({
				...child,
				node: retainPresentations(child.node),
			})),
		};
	}
	return {
		...node,
		childPresentations: node.childPresentations.map(retainPresentation),
		primaryPresentation: retainPresentation(node.primaryPresentation),
	};
}

function retainPresentation(
	presentation: WorkbenchSharedPresentation,
): WorkbenchSharedPresentation {
	return { ...presentation, intent: { kind: 'retain' } };
}
