/** Stands in for a store's subscription while there is no store to observe. */
export function subscribeToNothing(): () => void {
	return () => undefined;
}
