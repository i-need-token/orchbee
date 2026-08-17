/**
 * Source-neutral client surface for terminals whose authoritative VT state lives in Terminal Host.
 */

export { ProjectedHistoryPhase, ProjectedTerminal, ProjectedTerminalPhase } from './projected';
export type {
	ProjectedBrokerFailure,
	ProjectedFrameResult,
	ProjectedHistoryPhaseValue,
	ProjectedPasteResult,
	ProjectedPasteReview,
	ProjectedPasteSource,
	ProjectedRowsResult,
	ProjectedTerminalCommand,
	ProjectedTerminalCommandPort,
	ProjectedTerminalErrorEvent,
	ProjectedTerminalFrameTelemetry,
	ProjectedTerminalLabels,
	ProjectedTerminalLinkOpenRequest,
	ProjectedTerminalMetadata,
	ProjectedTerminalCloseActivity,
	ProjectedTerminalOptions,
	ProjectedTerminalPhaseValue,
	ProjectedTerminalSelectionCopyEvent,
	ProjectedTerminalSelectionCopyResult,
	ProjectedTerminalSelectionPoint,
	ProjectedTerminalSelectionRange,
	ProjectedTerminalSideEffectResult,
} from './projected';

export { TerminalBufferRowDetail } from './buffer-rows';
export type {
	BufferRowsResult,
	TerminalBufferCellMapping,
	TerminalBufferRenderCell,
	TerminalBufferRow,
	TerminalBufferRowDetail as TerminalBufferRowDetailValue,
	TerminalBufferRowReader,
	TerminalBufferRows,
	TerminalBufferRowsRequest,
	TerminalBufferRowsSnapshot,
} from './buffer-rows';

export type {
	ProjectedSearchWorkerFactory,
	ProjectedTerminalSearchEvent,
	TerminalSearchMatch,
	TerminalSearchOptions,
} from './search';

export {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	normalizeTerminalPresentationConfiguration,
} from './presentation-configuration';
export type {
	TerminalPresentationConfiguration,
	TerminalPresentationConfigurationUpdateResult,
	TerminalPresentationConfigurationValidationResult,
} from './presentation-configuration';

export type { RendererFrameReason, RendererFrameTelemetry, RendererHealthEvent } from './rendering';
export type {
	FontMetricModifier,
	FontMetricModifiers,
	FontSizeAdjustment,
	FontWeight,
	ITheme,
	PaddingBalance,
	RendererOptions,
	TerminalDefaultColorOverride,
} from './types';

export type { IDisposable, IEvent } from './EventEmitter';
export type { TerminalOpenTarget } from './terminal-contracts';

export {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION,
	isTerminalOptionAsAltMode,
	TerminalOptionAsAltMode,
} from './input-configuration';
export type { TerminalInputConfiguration } from './input-configuration';
