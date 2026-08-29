export {
	buildPreeditState,
	projectPreeditLayout,
	shouldSuppressComposingControlInput,
	type CompositionSelection,
	type PreeditCellData,
	type PreeditLayout,
	type PreeditLayoutRow,
	type PreeditState,
} from './preedit';
export {
	calculateInputAnchor,
	type InputAnchorLayout,
	type InputAnchorRequest,
} from './calculateInputAnchor';
export { OrderedInputDispatcher } from './OrderedInputDispatcher';
export { ghosttyCodepointWidth } from './ghosttyCodepointWidth';
export {
	classifySurfaceMousePlatform,
	resolveSurfaceMouseCursor,
	surfaceLinkModifierIsActive,
	type SurfaceMouseModifiers,
} from './surface-mouse';
export {
	createTerminalTouchGestureState,
	TERMINAL_TOUCH_ACTIVATION_DISTANCE_PX,
	transitionTerminalTouchGesture,
	type TerminalTouchGestureCommand,
	type TerminalTouchGestureEvent,
	type TerminalTouchGestureState,
	type TerminalTouchGestureTransition,
} from './terminal-touch-gesture';
export {
	beginTerminalComposition,
	createTerminalInputState,
	disposeTerminalInputState,
	finishTerminalComposition,
	observeTerminalBeforeInput,
	observeTerminalPaste,
	recordTerminalBeforeInput,
	recordTerminalComposition,
	recordTerminalKeyInput,
	recordTerminalMouseSample,
	recordTerminalPaste,
	repairTerminalInputState,
	supersedeCancelledTerminalComposition,
	TerminalBeforeInputOrigin,
	TerminalCompositionPhase,
	terminalCompositionUpdateIsEligible,
	type TerminalCompositionEndDecision,
	type TerminalBeforeInputDecision,
	type TerminalCompositionSettlement,
	type TerminalInputCorrelation,
	type TerminalInputDecision,
	type TerminalInputRepair,
	type TerminalInputState,
	type TerminalPasteCorrelation,
} from './terminal-input-state';
