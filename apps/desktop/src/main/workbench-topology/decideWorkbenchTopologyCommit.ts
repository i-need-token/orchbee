import {
	WorkbenchTopologyRevisionSchema,
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyRevision,
} from '@orchbee/workbench/topology';

type WorkbenchTopologyStoreUnavailableReason = 'invalid' | 'read-failed' | 'write-failed';

export type WorkbenchTopologyStoreLoadResult =
	| { readonly document: WorkbenchTopologyDocument; readonly kind: 'available' }
	| { readonly kind: 'missing' }
	| {
			readonly kind: 'unavailable';
			readonly reason: Exclude<WorkbenchTopologyStoreUnavailableReason, 'write-failed'>;
	  };

export type WorkbenchTopologyStoreCommitResult =
	| { readonly kind: 'committed'; readonly revision: WorkbenchTopologyRevision }
	| {
			readonly actualRevision: WorkbenchTopologyRevision | null;
			readonly kind: 'conflict';
	  }
	| { readonly kind: 'rejected'; readonly reason: 'revision-sequence' }
	| {
			readonly kind: 'unavailable';
			readonly reason: WorkbenchTopologyStoreUnavailableReason;
	  };

type WorkbenchTopologyCommitDecision =
	| WorkbenchTopologyStoreCommitResult
	| { readonly document: WorkbenchTopologyDocument; readonly kind: 'write' };

export interface WorkbenchTopologyStore {
	commit(
		expectedRevision: WorkbenchTopologyRevision | null,
		document: WorkbenchTopologyDocument,
	): Promise<WorkbenchTopologyStoreCommitResult>;
	load(): Promise<WorkbenchTopologyStoreLoadResult>;
}

export function decideWorkbenchTopologyCommit(
	loaded: WorkbenchTopologyStoreLoadResult,
	expectedRevision: WorkbenchTopologyRevision | null,
	document: WorkbenchTopologyDocument,
): WorkbenchTopologyCommitDecision {
	if (loaded.kind === 'unavailable') return loaded;
	if (document.revision !== nextWorkbenchTopologyRevision(expectedRevision)) {
		return { kind: 'rejected', reason: 'revision-sequence' };
	}
	const actualRevision = loaded.kind === 'available' ? loaded.document.revision : null;
	if (actualRevision !== expectedRevision) return { actualRevision, kind: 'conflict' };
	return { document, kind: 'write' };
}

function nextWorkbenchTopologyRevision(
	revision: WorkbenchTopologyRevision | null,
): WorkbenchTopologyRevision {
	if (revision === null) return workbenchTopologyRevision('0');
	const canonical = WorkbenchTopologyRevisionSchema.parse(revision);
	return workbenchTopologyRevision((BigInt(canonical) + 1n).toString());
}
