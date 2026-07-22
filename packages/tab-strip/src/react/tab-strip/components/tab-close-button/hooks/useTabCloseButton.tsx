import { useRef, type MouseEvent, type PointerEvent, type ReactNode } from 'react';
import { shouldTabCloseButtonHandlePointer } from '../../../../../tab';
import { TabStripIcon } from '../../tab-strip-icon/TabStripIcon';
interface UseTabCloseButtonOptions {
	tabTitle: string;
	touchUi?: boolean;
	onClose: (event: MouseEvent<HTMLButtonElement>, pointerType: string) => void;
}
export function useTabCloseButton({
	tabTitle: _tabTitle,
	touchUi = false,
	onClose,
}: UseTabCloseButtonOptions): ReactNode {
	const lastPointerType = useRef('mouse');
	const handlePointerDown = (event: PointerEvent<HTMLButtonElement>) => {
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
		) {
			event.stopPropagation();
		}
	};
	return (
		<button
			type='button'
			className='ct-tab__close'
			aria-label='Close'
			tabIndex={-1}
			onPointerDown={handlePointerDown}
			onClick={(event: MouseEvent<HTMLButtonElement>) => {
				event.stopPropagation();
				onClose(event, lastPointerType.current);
			}}
		>
			<span className='ct-tab__close-target'>
				<TabStripIcon name='close' />
			</span>
		</button>
	);
}
