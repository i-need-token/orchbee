/** A plain primary double-click: the primary button with no modifier key held. */
export function isPlainPrimaryDoubleClick(
	event: Pick<MouseEvent, 'altKey' | 'button' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
): boolean {
	return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}
