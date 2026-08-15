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
			readonly mode: 'development-host-restart' | 'preserve' | 'stop';
	  }
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'disposed' };

export type DesktopClientDocumentProposalDecision =
	| { readonly effect: 'defer' | 'reconcile'; readonly kind: 'commit' }
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
		if (canonical === null) {
			return { kind: 'rejected', reason: 'exit-transition' };
		}
		let effect: 'defer' | 'reconcile' | null = null;
		if (isStopProgression(canonical, proposal.document)) {
			effect = 'reconcile';
		} else if (
			phase.mode === 'development-host-restart' &&
			isDevelopmentHostRestartStaging(canonical, proposal.document)
		) {
			effect = 'defer';
		}
		if (effect === null) return { kind: 'rejected', reason: 'exit-transition' };
		if (proposal.document.revision !== nextRevision(proposal.expectedRevision)) {
			return { kind: 'rejected', reason: 'revision-sequence' };
		}
		if (proposal.expectedRevision !== canonical.revision) {
			return { document: canonical, kind: 'stale' };
		}
		return { effect, kind: 'commit' };
	}
	if (proposal.document.revision !== nextRevision(proposal.expectedRevision)) {
		return { kind: 'rejected', reason: 'revision-sequence' };
	}
	if (canonical === null) {
		return proposal.expectedRevision === null
			? { effect: 'reconcile', kind: 'commit' }
			: { document: proposal.document, kind: 'stale' };
	}
	if (proposal.expectedRevision === canonical.revision) {
		return { effect: 'reconcile', kind: 'commit' };
	}
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

function isDevelopmentHostRestartStaging(
	canonical: WorkbenchSharedDocument,
	candidate: WorkbenchSharedDocument,
): boolean {
	if (
		developmentRestartStructureFingerprint(canonical) !==
		developmentRestartStructureFingerprint(candidate)
	) {
		return false;
	}
	const canonicalIds = new Set<string>();
	const canonicalOperationIds = new Set<string>();
	const replacementOperationIds = new Set<string>();
	const canonicalRecords = orderedPresentationRecords(canonical);
	const candidateRecords = orderedPresentationRecords(candidate);
	if (canonicalRecords.length !== candidateRecords.length) return false;
	for (const record of canonicalRecords) {
		canonicalIds.add(record.presentation.presentationId);
		const operationId = presentationOperationId(record.presentation.intent);
		if (operationId !== null) canonicalOperationIds.add(operationId);
	}
	let replacementCount = 0;
	for (const [index, before] of canonicalRecords.entries()) {
		const after = candidateRecords[index];
		if (after === undefined || after.contextId !== before.contextId) return false;
		if (before.presentation.presentationId === after.presentation.presentationId) {
			if (
				JSON.stringify(before.presentation.intent) !== JSON.stringify(after.presentation.intent)
			) {
				return false;
			}
			continue;
		}
		if (
			before.presentation.intent.kind !== 'stopped' ||
			after.presentation.intent.kind !== 'provision' ||
			canonicalIds.has(after.presentation.presentationId) ||
			canonicalOperationIds.has(after.presentation.intent.operationId) ||
			replacementOperationIds.has(after.presentation.intent.operationId)
		) {
			return false;
		}
		replacementOperationIds.add(after.presentation.intent.operationId);
		replacementCount += 1;
	}
	if (replacementCount === 0) return false;
	for (const beforeContext of canonical.terminalContexts) {
		const afterContext = candidate.terminalContexts.find(
			(context) => context.terminalContextId === beforeContext.terminalContextId,
		);
		if (
			afterContext === undefined ||
			afterContext.panelWorkspace.nextTerminalSequence !==
				beforeContext.panelWorkspace.nextTerminalSequence
		) {
			return false;
		}
	}
	return true;
}

function presentationOperationId(intent: WorkbenchSharedPresentationIntent): string | null {
	return intent.kind === 'provision' || intent.kind === 'stop' || intent.kind === 'close'
		? intent.operationId
		: null;
}

function orderedPresentationRecords(document: WorkbenchSharedDocument): Array<{
	readonly contextId: string;
	readonly presentation: WorkbenchSharedPresentation;
}> {
	const records: Array<{
		readonly contextId: string;
		readonly presentation: WorkbenchSharedPresentation;
	}> = [];
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) {
			collectOrderedPresentationRecords(tab.root, context.terminalContextId, records);
		}
	}
	return records;
}

function collectOrderedPresentationRecords(
	node: WorkbenchSharedPanelNode,
	contextId: string,
	records: Array<{
		readonly contextId: string;
		readonly presentation: WorkbenchSharedPresentation;
	}>,
): void {
	if (node.kind === 'split') {
		for (const child of node.children) {
			collectOrderedPresentationRecords(child.node, contextId, records);
		}
		return;
	}
	for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
		records.push({ contextId, presentation });
	}
}

function developmentRestartStructureFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify({
		projectWorkspace: document.projectWorkspace,
		terminalContexts: document.terminalContexts.map((context) => ({
			...context,
			panelWorkspace: {
				...context.panelWorkspace,
				nextTerminalSequence: null,
				tabs: context.panelWorkspace.tabs.map((tab) => ({
					...tab,
					root: developmentRestartStructureNode(tab.root),
				})),
			},
		})),
		version: document.version,
	});
}

function developmentRestartStructureNode(node: WorkbenchSharedPanelNode): unknown {
	if (node.kind === 'split') {
		return {
			...node,
			children: node.children.map((child) => ({
				...child,
				node: developmentRestartStructureNode(child.node),
			})),
		};
	}
	return {
		...node,
		childPresentations: node.childPresentations.map(developmentRestartPresentation),
		primaryPresentation: developmentRestartPresentation(node.primaryPresentation),
	};
}

function developmentRestartPresentation(
	presentation: WorkbenchSharedPresentation,
): Omit<WorkbenchSharedPresentation, 'intent' | 'presentationId'> {
	return { launchCwd: presentation.launchCwd, title: presentation.title };
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
