export {
	WorkbenchSharedDocumentSchema,
	WorkbenchSharedDocumentRevisionSchema,
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	parseWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from './workbench-document';
export type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentInput,
	WorkbenchSharedPanelNode,
	WorkbenchSharedPresentation,
	WorkbenchSharedPresentationIntent,
	WorkbenchSharedDocumentRevision,
	WorkbenchSharedTerminalContext,
} from './workbench-document';
export {
	WorkbenchClientViewSchema,
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	parseWorkbenchClientView,
	reconcileWorkbenchClientView,
} from './workbench-client-view';
export type { WorkbenchClientView } from './workbench-client-view';
