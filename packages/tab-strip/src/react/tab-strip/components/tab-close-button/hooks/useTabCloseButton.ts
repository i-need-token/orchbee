import { useRef, type MouseEvent, type PointerEvent } from 'react';
import { shouldTabCloseButtonHandlePointer } from '../../../../../tab';

interface UseTabCloseButtonOptions {
	touchUi?: boolean;
	onClose: (event: MouseEvent<HTMLButtonElement>, pointerType: string) => void;
}

interface TabCloseButtonView {
	onPointerDown: (event: PointerEvent<HTMLButtonElement>) => void;
	onClick: (event: MouseEvent<HTMLButtonElement>) => void;
}

export function useTabCloseButton({
	touchUi = false,
	onClose,
}: UseTabCloseButtonOptions): TabCloseButtonView {
	const lastPointerType = useRef('mouse');
	return {
		onPointerDown: (event) => {
			lastPointerType.current = event.pointerType || 'mouse';
			// Views::TabCloseButton explicitly does not take focus on right click.
			// Keep propagation intact so the parent tab can still open its menu.
			if (event.button === 2) event.preventDefault();
			const rect = event.currentTarget.getBoundingClientRect();
			if (
				shouldTabCloseButtonHandlePointer({
					button: event.button,
					pointerType: event.pointerType,
					x: event.clientX - rect.left,
					y: event.clientY - rect.top,
					touchUi,
				})
			)
				event.stopPropagation();
		},
		onClick: (event) => {
			event.stopPropagation();
			onClose(event, lastPointerType.current);
		},
	};
}
