import type { CSSProperties, ReactNode } from 'react';
import { DROP_ARROW_SIZE, DropArrowDirection, type DropArrowPlacement } from '../../../../shared';
export interface DropArrowProps {
	placement: Readonly<DropArrowPlacement>;
	className?: string;
}
const ROTATION: Record<DropArrowDirection, number> = {
	[DropArrowDirection.Down]: 0,
	[DropArrowDirection.Left]: 90,
	[DropArrowDirection.Up]: 180,
	[DropArrowDirection.Right]: 270,
};
export function DropArrow({ placement, className = '' }: DropArrowProps): ReactNode {
	const style: CSSProperties = {
		position: 'fixed',
		left: placement.bounds.x,
		top: placement.bounds.y,
		width: placement.bounds.width || DROP_ARROW_SIZE,
		height: placement.bounds.height || DROP_ARROW_SIZE,
		transform: `rotate(${ROTATION[placement.direction]}deg)`,
	};
	return (
		<svg
			className={`ct-drop-arrow ${className}`.trim()}
			style={style}
			viewBox='0 0 20 20'
			aria-hidden='true'
		>
			<path d='M4 7.5 10 13.5 16 7.5 13.8 5.3 10 9.1 6.2 5.3Z' fill='currentColor' />
		</svg>
	);
}
