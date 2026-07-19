export { InputHandler, type InputHandlerDependencies } from './InputHandler';
export {
	calculateInputAnchor,
	type InputAnchorLayout,
	type InputAnchorRequest,
} from './calculateInputAnchor';
export {
	KeyEventRegistry,
	type TerminalKeyEventContext,
	type TerminalKeyEventHandler,
} from './KeyEventRegistry';
export { OrderedInputDispatcher } from './OrderedInputDispatcher';
export {
	encodePaste,
	inspectPaste,
	type PasteCodec,
	type PasteInspection,
	type PastePolicyOptions,
	type PasteResult,
	type PasteSource,
} from './paste-policy';
export {
	buildPreeditState,
	shouldSuppressComposingControlInput,
	type CompositionSelection,
	type PreeditCellData,
	type PreeditState,
} from './preedit';
export {
	classifySurfaceMousePlatform,
	isSurfaceMouseModifierKey,
	resolveSurfaceMouseCursor,
	type SurfaceMouseCursor,
	type SurfaceMouseCursorInput,
	type SurfaceMouseModifiers,
	type SurfaceMousePlatform,
} from './surface-mouse';
