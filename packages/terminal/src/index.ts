/**
 * Source-neutral client surface for terminals whose authoritative VT state lives in Terminal Host.
 */

export { ProjectedHistoryPhase, ProjectedTerminal, ProjectedTerminalPhase } from './projected';
export type {
	ProjectedBrokerFailure,
	ProjectedCommandOutput,
	ProjectedCommandOutputReadResult,
	ProjectedScrollback,
	ProjectedScrollbackReadResult,
	ProjectedFrameResult,
	ProjectedHistoryPhaseValue,
	ProjectedPasteResult,
	ProjectedPasteReview,
	ProjectedPasteSource,
	ProjectedRowsResult,
	ProjectedTerminalActionContext,
	ProjectedTerminalActionContextRequest,
	ProjectedTerminalCommand,
	ProjectedTerminalCommandPort,
	ProjectedTerminalErrorEvent,
	ProjectedTerminalFrameTelemetry,
	ProjectedTerminalInputCapability,
	ProjectedTerminalInputCapabilityUpdateResult,
	TerminalImagePreviewAnchor,
	TerminalImagePreviewMediaType,
	TerminalImagePreviewPresentation,
	ProjectedTerminalInjectedInput,
	ProjectedTerminalInjectedInputReadiness,
	ProjectedTerminalInjectedInputResult,
	ProjectedTerminalInjectedKeyModifiers,
	ProjectedTerminalLabels,
	ProjectedTerminalLinkOpenRequest,
	ProjectedTerminalMetadata,
	ProjectedTerminalCloseActivity,
	ProjectedTerminalOptions,
	ProjectedTerminalPhaseValue,
	ProjectedTerminalPresentationSnapshot,
	ProjectedTerminalSelectionCopyEvent,
	ProjectedTerminalSelectionCopyResult,
	ProjectedTerminalSelectionPoint,
	ProjectedTerminalSelectionRange,
	ProjectedTerminalSideEffectResult,
	ProjectedTerminalThemePreparationResult,
	ProjectedTerminalThemeReadinessResult,
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

export type {
	RendererFrameReason,
	RendererFrameTelemetry,
	RendererHealthEvent,
	TerminalThemeOverlayPresentation,
} from './rendering';
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
export type { TerminalHint, TerminalHintChoiceResult, TerminalHintKind } from './terminal-hints';
export { normalizePermittedExternalTerminalUrl } from './normalizePermittedExternalTerminalUrl';

export { measureMountedTerminalGeometry } from './measureMountedTerminalGeometry';
export type { MountedTerminalGeometryOptions } from './measureMountedTerminalGeometry';

export {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION,
	isTerminalOptionAsAltMode,
	TerminalOptionAsAltMode,
} from './input-configuration';
export type { TerminalInputConfiguration } from './input-configuration';

export {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION,
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	normalizeTerminalInteractionConfiguration,
} from './interaction-configuration';
export type {
	TerminalInteractionConfiguration,
	TerminalInteractionConfigurationSource,
	TerminalInteractionConfigurationValidationResult,
} from './interaction-configuration';
