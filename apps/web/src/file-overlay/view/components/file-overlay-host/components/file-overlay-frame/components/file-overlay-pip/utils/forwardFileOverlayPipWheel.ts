import { wheelEnvironment, wheelScrollDelta } from '../../../../../../../../../list-scrolling';

const SCROLLING_OVERFLOW = new Set(['auto', 'scroll']);

/** The legacy notch deltas Chromium reports beside the standard ones, which editors read first. */
function legacyDeltas(wheel: WheelEvent): { wheelDeltaX?: number; wheelDeltaY?: number } {
	return {
		...('wheelDeltaX' in wheel && typeof wheel.wheelDeltaX === 'number'
			? { wheelDeltaX: wheel.wheelDeltaX }
			: {}),
		...('wheelDeltaY' in wheel && typeof wheel.wheelDeltaY === 'number'
			? { wheelDeltaY: wheel.wheelDeltaY }
			: {}),
	};
}

/** The content's element under the pointer, which inert content keeps from hit testing. */
function contentAt(content: HTMLElement, clientX: number, clientY: number): Element | undefined {
	const inert = content.inert;
	content.inert = false;
	try {
		return content.ownerDocument
			.elementsFromPoint(clientX, clientY)
			.find((element) => element !== content && content.contains(element));
	} finally {
		content.inert = inert;
	}
}

/**
 * Hands a wheel turned over picture-in-picture's click shield to the content beneath, as though the
 * shield were not there: the element under the pointer hears the same wheel, so an editor scrolls
 * as it does under the pointer. When nothing there takes the wheel, the nearest element that
 * scrolls moves as a list does.
 */
export function forwardFileOverlayPipWheel(content: HTMLElement, wheel: WheelEvent): void {
	const view = content.ownerDocument.defaultView;
	const target = contentAt(content, wheel.clientX, wheel.clientY);
	if (view === null || target === undefined) return;
	const init = {
		...legacyDeltas(wheel),
		altKey: wheel.altKey,
		bubbles: true,
		cancelable: true,
		clientX: wheel.clientX,
		clientY: wheel.clientY,
		ctrlKey: wheel.ctrlKey,
		deltaMode: wheel.deltaMode,
		deltaX: wheel.deltaX,
		deltaY: wheel.deltaY,
		deltaZ: wheel.deltaZ,
		metaKey: wheel.metaKey,
		screenX: wheel.screenX,
		screenY: wheel.screenY,
		shiftKey: wheel.shiftKey,
		view,
	};
	if (!target.dispatchEvent(new WheelEvent('wheel', init))) return;
	const delta = wheelScrollDelta(
		wheel,
		wheelEnvironment(view.navigator.userAgent),
		view.devicePixelRatio,
	);
	if (delta === 0) return;
	for (
		let element: Element | null = target;
		element !== null && content.contains(element);
		element = element.parentElement
	) {
		if (
			element.scrollHeight > element.clientHeight &&
			SCROLLING_OVERFLOW.has(view.getComputedStyle(element).overflowY)
		) {
			element.scrollTop += delta;
			return;
		}
	}
}
