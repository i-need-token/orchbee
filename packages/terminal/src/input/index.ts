export {
	buildPreeditState,
	projectPreeditLayout,
	shouldSuppressComposingControlInput,
	type PreeditCellData,
	type PreeditLayout,
	type PreeditLayoutRow,
	type PreeditState,
} from './preedit';
export { calculateInputAnchor } from './calculateInputAnchor';
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
	transitionTerminalTouchGesture,
	type TerminalTouchGestureCommand,
	type TerminalTouchGestureEvent,
	type TerminalTouchGestureState,
} from './terminal-touch-gesture';
export {
	beginTerminalComposition,
	createTerminalInputState,
	disposeTerminalInputState,
	finishTerminalComposition,
	observeTerminalBeforeInput,
	observeTerminalPaste,
	recordTerminalKeyInput,
	recordTerminalMouseSample,
	recordTerminalPaste,
	repairTerminalInputState,
	supersedeCancelledTerminalComposition,
	TerminalBeforeInputOrigin,
	TerminalCompositionPhase,
	terminalCompositionUpdateIsEligible,
	type TerminalInputState,
} from './terminal-input-state';
