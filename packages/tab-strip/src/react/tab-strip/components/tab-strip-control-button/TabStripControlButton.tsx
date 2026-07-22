import type { ButtonHTMLAttributes, CSSProperties, ReactNode } from 'react';
import { getTabStripControlButtonRadii, TabStripControlButtonEdge } from '../../../../shared';
export interface TabStripControlButtonProps extends Omit<
	ButtonHTMLAttributes<HTMLButtonElement>,
	'children'
> {
	icon?: ReactNode;
	label?: ReactNode;
	fixedFlatEdge?: TabStripControlButtonEdge;
	animatedFlatEdge?: TabStripControlButtonEdge;
	flatEdgeFactor?: number;
	leftCornerRadius?: number;
	rightCornerRadius?: number;
	flatCornerRadius?: number;
	frameCondensed?: boolean;
}
export function TabStripControlButton({
	icon,
	label,
	fixedFlatEdge = TabStripControlButtonEdge.None,
	animatedFlatEdge = TabStripControlButtonEdge.None,
	flatEdgeFactor = 1,
	leftCornerRadius,
	rightCornerRadius,
	flatCornerRadius,
	frameCondensed = false,
	className = '',
	style,
	...buttonProps
}: TabStripControlButtonProps): ReactNode {
	const radii = getTabStripControlButtonRadii({
		fixedFlatEdge,
		animatedFlatEdge,
		flatEdgeFactor,
		frameCondensed,
		...(leftCornerRadius === undefined
			? {}
			: {
					leftCornerRadius,
				}),
		...(rightCornerRadius === undefined
			? {}
			: {
					rightCornerRadius,
				}),
		...(flatCornerRadius === undefined
			? {}
			: {
					flatCornerRadius,
				}),
	});
	const computedStyle: CSSProperties = {
		borderTopLeftRadius: radii.topLeft,
		borderTopRightRadius: radii.topRight,
		borderBottomRightRadius: radii.bottomRight,
		borderBottomLeftRadius: radii.bottomLeft,
		...style,
	};
	return (
		<button
			{...buttonProps}
			tabIndex={buttonProps.tabIndex ?? -1}
			className={`ct-tab-strip-control-button ${className}`.trim()}
			data-frame-condensed={frameCondensed || undefined}
			data-fixed-flat-edge={fixedFlatEdge}
			data-animated-flat-edge={animatedFlatEdge}
			style={computedStyle}
		>
			{icon ? (
				<span className='ct-tab-strip-control-button__icon' aria-hidden='true'>
					{icon}
				</span>
			) : null}
			{label}
		</button>
	);
}
