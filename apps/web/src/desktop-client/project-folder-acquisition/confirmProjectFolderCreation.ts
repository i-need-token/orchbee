import {
	matchesWorkbenchProjectCreation,
	type WorkbenchProjectCreationReceipt,
	type WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

interface ProjectPublication {
	readonly committedDocument: WorkbenchSharedDocument | null;
	flush(): Promise<
		| { readonly kind: 'flushed'; readonly revision: WorkbenchSharedDocumentRevision }
		| { readonly kind: 'unavailable'; readonly reason: string }
	>;
}

export async function confirmProjectFolderCreation(
	publication: ProjectPublication,
	runtime: WorkbenchProjectWorkspaceRuntime,
	receipt: WorkbenchProjectCreationReceipt,
): Promise<boolean> {
	const result = await publication.flush();
	const document = publication.committedDocument;
	return (
		result.kind === 'flushed' &&
		document !== null &&
		matchesWorkbenchProjectCreation(document, receipt) &&
		matchesWorkbenchProjectCreation(runtime.getSharedDocument(document.revision), receipt)
	);
}
