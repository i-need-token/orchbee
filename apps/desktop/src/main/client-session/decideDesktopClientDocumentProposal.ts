import {
	WorkbenchSharedDocumentRevisionSchema,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedPresentation,
	type WorkbenchSharedPresentationIntent,
} from '@orchbee/workbench/document';

export type DesktopClientSessionPhase =
	| { readonly kind: 'accepting' }
	| {
			readonly exitOwner: string;
			readonly kind: 'quiescing';
			readonly mode: 'preserve' | 'stop';
	  }
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'disposed' };

export type DesktopClientDocumentProposalDecision =
	| { readonly kind: 'commit' }
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'converged' }
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'stale' }
	| {
			readonly kind: 'rejected';
			readonly reason:
				| 'application-quiescing'
				| 'control-unavailable'
				| 'exit-transition'
				| 'revision-sequence';
	  };

export interface DesktopClientDocumentProposal {
	readonly clientSessionId: string;
	readonly document: WorkbenchSharedDocument;
	readonly expectedRevision: WorkbenchSharedDocumentRevision | null;
}

export function decideDesktopClientDocumentProposal(
	canonical: WorkbenchSharedDocument | null,
	proposal: DesktopClientDocumentProposal,
	phase: DesktopClientSessionPhase,
): DesktopClientDocumentProposalDecision {
	if (phase.kind === 'disposed' || phase.kind === 'unavailable') {
		return { kind: 'rejected', reason: 'control-unavailable' };
	}
	if (phase.kind === 'quiescing') {
		if (phase.mode === 'preserve' || phase.exitOwner !== proposal.clientSessionId) {
			return { kind: 'rejected', reason: 'application-quiescing' };
		}
		if (canonical === null || !isStopProgression(canonical, proposal.document)) {
			return { kind: 'rejected', reason: 'exit-transition' };
		}
	}
	if (proposal.document.revision !== nextRevision(proposal.expectedRevision)) {
		return { kind: 'rejected', reason: 'revision-sequence' };
	}
	if (canonical === null) {
		return proposal.expectedRevision === null
			? { kind: 'commit' }
			: { document: proposal.document, kind: 'stale' };
	}
	if (proposal.expectedRevision === canonical.revision) return { kind: 'commit' };
	return documentContentFingerprint(proposal.document) === documentContentFingerprint(canonical)
		? { document: canonical, kind: 'converged' }
		: { document: canonical, kind: 'stale' };
}

function documentContentFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function nextRevision(
	revision: WorkbenchSharedDocumentRevision | null,
): WorkbenchSharedDocumentRevision {
	if (revision === null) return workbenchSharedDocumentRevision('0');
	const canonical = WorkbenchSharedDocumentRevisionSchema.parse(revision);
	return workbenchSharedDocumentRevision((BigInt(canonical) + 1n).toString());
}

function isStopProgression(
	canonical: WorkbenchSharedDocument,
	candidate: WorkbenchSharedDocument,
): boolean {
	const canonicalPresentations = presentationRecords(canonical);
	const candidatePresentations = presentationRecords(candidate);
	if (canonicalPresentations.size !== candidatePresentations.size) return false;
	if (documentStructureFingerprint(canonical) !== documentStructureFingerprint(candidate)) {
		return false;
	}
	let changed = false;
	for (const [presentationId, before] of canonicalPresentations) {
		const after = candidatePresentations.get(presentationId);
		if (after === undefined) return false;
		if (JSON.stringify(before) === JSON.stringify(after)) continue;
		if (!allowedStopIntentTransition(before.kind, after.kind)) return false;
		changed = true;
	}
	return changed;
}

function allowedStopIntentTransition(before: string, after: string): boolean {
	return (
		((before === 'provision' || before === 'retain') && after === 'stop') ||
		(before === 'stop' && after === 'stopped')
	);
}

function presentationRecords(
	document: WorkbenchSharedDocument,
): Map<string, WorkbenchSharedPresentationIntent> {
	const records = new Map<string, WorkbenchSharedPresentationIntent>();
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) collectPresentationRecords(tab.root, records);
	}
	return records;
}

function collectPresentationRecords(
	node: WorkbenchSharedPanelNode,
	records: Map<string, WorkbenchSharedPresentationIntent>,
): void {
	if (node.kind === 'split') {
		for (const child of node.children) collectPresentationRecords(child.node, records);
		return;
	}
	for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
		records.set(presentation.presentationId, presentation.intent);
	}
}

function documentStructureFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify({
		projectWorkspace: document.projectWorkspace,
		terminalContexts: document.terminalContexts.map((context) => ({
			...context,
			panelWorkspace: {
				...context.panelWorkspace,
				tabs: context.panelWorkspace.tabs.map((tab) => ({
					...tab,
					root: presentationIntentFreeNode(tab.root),
				})),
			},
		})),
		version: document.version,
	});
}

function presentationIntentFreeNode(node: WorkbenchSharedPanelNode): unknown {
	if (node.kind === 'split') {
		return {
			...node,
			children: node.children.map((child) => ({
				...child,
				node: presentationIntentFreeNode(child.node),
			})),
		};
	}
	return {
		...node,
		childPresentations: node.childPresentations.map(presentationWithoutIntent),
		primaryPresentation: presentationWithoutIntent(node.primaryPresentation),
	};
}

function presentationWithoutIntent(
	presentation: WorkbenchSharedPresentation,
): Omit<WorkbenchSharedPresentation, 'intent'> {
	return {
		launchCwd: presentation.launchCwd,
		presentationId: presentation.presentationId,
		title: presentation.title,
	};
}
