/**
 * Whether a Panel this Client asked for takes the keyboard. A keypress or click since the request
 * began, a Tab no longer shown, a place the Client has left, or a cancelled request each keep the
 * focus where it is.
 */
export function takesCreatedPanelFocus({
	handoverHeld,
	sameActiveTab,
	sameLocation,
	signalAborted,
}: {
	readonly handoverHeld: boolean;
	readonly sameActiveTab: boolean;
	readonly sameLocation: boolean;
	readonly signalAborted: boolean;
}): boolean {
	return handoverHeld && sameActiveTab && sameLocation && !signalAborted;
}
