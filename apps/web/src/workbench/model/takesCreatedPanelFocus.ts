/**
 * Whether a Panel this Client asked for takes the keyboard. The Session's split preference, read when
 * the request began, must allow it; a keypress or click since then, a Tab no longer shown, a place the
 * Client has left, or a cancelled request each keep the focus where it is.
 */
export function takesCreatedPanelFocus({
	focusNewPanel,
	handoverHeld,
	sameActiveTab,
	sameLocation,
	signalAborted,
}: {
	readonly focusNewPanel: boolean;
	readonly handoverHeld: boolean;
	readonly sameActiveTab: boolean;
	readonly sameLocation: boolean;
	readonly signalAborted: boolean;
}): boolean {
	return focusNewPanel && handoverHeld && sameActiveTab && sameLocation && !signalAborted;
}
