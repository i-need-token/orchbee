export { TabStripObserverList, type TabStripObserver } from './TabStripObserverList';
export {
	DROP_ARROW_SIZE,
	DropArrowDirection,
	DropRelativeToIndex,
	DropGroupInclusion,
	DropArrowState,
	type TabDropIndex,
	type DropArrowPlacement,
	type DropArrowBoundsCallback,
} from './drop-arrow';
export {
	ROUNDED_SCROLLBAR_THUMB_THICKNESS,
	ROUNDED_SCROLLBAR_TRAILING_PADDING,
	ROUNDED_SCROLLBAR_FADE_DELAY_MS,
	ROUNDED_SCROLLBAR_FADE_DURATION_MS,
	roundedScrollbarThickness,
	calculateRoundedScrollbarThumb,
	type RoundedScrollbarThumbGeometry,
} from './rounded-scrollbar';
export {
	TAB_STRIP_COMBO_ANIMATION_DURATION_MS,
	TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS,
	TabStripComboContext,
	TabStripComboStartKind,
	getTabStripComboAnimationFactors,
	TabStripComboButtonController,
	type TabStripComboButtonId,
	type TabStripComboAnimationFactors,
	type TabStripComboButtonSnapshot,
	type TabStripComboButtonControllerOptions,
} from './tab-strip-combo-button';
export {
	TAB_STRIP_CONTROL_BUTTON_SIZE,
	TAB_STRIP_CONTROL_BUTTON_ICON_SIZE,
	TAB_STRIP_CONTROL_BUTTON_CORNER_RADIUS,
	TAB_STRIP_CONTROL_BUTTON_FLAT_CORNER_RADIUS,
	TabStripControlButtonEdge,
	getScaledTabStripControlButtonRadius,
	getTabStripControlButtonRadii,
	getTabStripControlButtonPreferredSize,
	type TabStripControlButtonRadii,
	type TabStripControlButtonInsets,
	type TabStripControlButtonShapeInput,
} from './tab-strip-control-button';
export {
	TAB_STRIP_FLAT_EDGE_BUTTON_SIZE,
	TAB_STRIP_FLAT_EDGE_BUTTON_ICON_SIZE,
	TAB_STRIP_FLAT_EDGE_BUTTON_PADDING,
	TAB_STRIP_FLAT_EDGE_RADIUS,
	TAB_STRIP_ROUNDED_EDGE_RADIUS,
	TabStripFlatEdge,
	getFlatEdgeButtonCornerRadii,
	getFlatEdgeButtonPreferredSize,
	getComboButtonFlatEdges,
	type CornerRadii,
	type FlatEdgeButtonSize,
} from './tab-strip-flat-edge-button';
