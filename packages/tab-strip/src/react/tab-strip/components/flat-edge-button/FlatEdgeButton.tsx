import type { ButtonHTMLAttributes, CSSProperties, ReactNode } from 'react';
import { TabStripOrientation } from '../../../../common';
import { getFlatEdgeButtonCornerRadii, TabStripFlatEdge } from '../../../../shared';

export interface FlatEdgeButtonProps extends Omit<
	ButtonHTMLAttributes<HTMLButtonElement>,
	'children'
> {
	icon: ReactNode;
	label: string;
	showLabel?: boolean;
	flatEdge?: TabStripFlatEdge;
	flatEdgeFactor?: number;
	expansionOrientation?: TabStripOrientation;
}

export function FlatEdgeButton({
	icon,
	label,
	showLabel = false,
	flatEdge = TabStripFlatEdge.None,
	flatEdgeFactor = 1,
	expansionOrientation = TabStripOrientation.Horizontal,
	className = '',
	style,
	...buttonProps
}: FlatEdgeButtonProps): ReactNode {
	const radii = getFlatEdgeButtonCornerRadii(flatEdge, flatEdgeFactor);
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
			className={`ct-flat-edge-button ${className}`.trim()}
			style={computedStyle}
			data-flat-edge={flatEdge}
			data-expansion-orientation={expansionOrientation}
		>
			<span className='ct-flat-edge-button__icon' aria-hidden='true'>
				{icon}
			</span>
			{showLabel ? <span className='ct-flat-edge-button__label'>{label}</span> : null}
		</button>
	);
}
