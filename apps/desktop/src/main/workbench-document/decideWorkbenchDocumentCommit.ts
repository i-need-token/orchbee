import {
	WorkbenchSharedDocumentRevisionSchema,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

type WorkbenchDocumentStoreUnavailableReason = 'invalid' | 'read-failed' | 'write-failed';

export type WorkbenchDocumentStoreLoadResult =
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'available' }
	| { readonly kind: 'missing' }
	| {
			readonly kind: 'unavailable';
			readonly reason: Exclude<WorkbenchDocumentStoreUnavailableReason, 'write-failed'>;
	  };

export type WorkbenchDocumentStoreCommitResult =
	| { readonly kind: 'committed'; readonly revision: WorkbenchSharedDocumentRevision }
	| {
			readonly actualRevision: WorkbenchSharedDocumentRevision | null;
			readonly kind: 'conflict';
	  }
	| { readonly kind: 'rejected'; readonly reason: 'revision-sequence' }
	| {
			readonly kind: 'unavailable';
			readonly reason: WorkbenchDocumentStoreUnavailableReason;
	  };

type WorkbenchDocumentCommitDecision =
	| WorkbenchDocumentStoreCommitResult
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'write' };

export interface WorkbenchDocumentStore {
	commit(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<WorkbenchDocumentStoreCommitResult>;
	load(): Promise<WorkbenchDocumentStoreLoadResult>;
}

export function decideWorkbenchDocumentCommit(
	loaded: WorkbenchDocumentStoreLoadResult,
	expectedRevision: WorkbenchSharedDocumentRevision | null,
	document: WorkbenchSharedDocument,
): WorkbenchDocumentCommitDecision {
	if (loaded.kind === 'unavailable') return loaded;
	if (document.revision !== nextWorkbenchSharedDocumentRevision(expectedRevision)) {
		return { kind: 'rejected', reason: 'revision-sequence' };
	}
	const actualRevision = loaded.kind === 'available' ? loaded.document.revision : null;
	if (actualRevision !== expectedRevision) return { actualRevision, kind: 'conflict' };
	return { document, kind: 'write' };
}

function nextWorkbenchSharedDocumentRevision(
	revision: WorkbenchSharedDocumentRevision | null,
): WorkbenchSharedDocumentRevision {
	if (revision === null) return workbenchSharedDocumentRevision('0');
	const canonical = WorkbenchSharedDocumentRevisionSchema.parse(revision);
	return workbenchSharedDocumentRevision((BigInt(canonical) + 1n).toString());
}
