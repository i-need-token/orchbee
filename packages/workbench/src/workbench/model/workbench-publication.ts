import type {
	TerminalPresentationLifecycle,
	WorkbenchSharedDocument,
} from '@orchbee/workbench-client-protocol';

/** The shared facts of one terminal that a document has no field for. */
interface WorkbenchPublishedPresentation {
	readonly customTitle: string | null;
	readonly lifecycle: TerminalPresentationLifecycle;
}

/**
 * One state published by an authority outside this runtime. The runtime takes all of it as shared
 * truth and keeps only what its own Client owns: selection, focus, and what a mounted terminal
 * reports about itself.
 */
export interface WorkbenchPublication {
	/** Changes exactly when a context does, so an unchanged context keeps its live models. */
	readonly contextFingerprints: ReadonlyMap<string, string>;
	readonly document: WorkbenchSharedDocument;
	/** Holds every terminal of the document, by presentation ID. */
	readonly presentations: ReadonlyMap<string, WorkbenchPublishedPresentation>;
	/** Holds every Split of the document, by Split ID, with weights that total 100. */
	readonly splitWeights: ReadonlyMap<string, readonly [first: number, second: number]>;
}
