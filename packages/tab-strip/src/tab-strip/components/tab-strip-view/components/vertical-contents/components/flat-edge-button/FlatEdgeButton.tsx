import styles from './FlatEdgeButton.module.css';
import type { MouseEventHandler, ReactNode } from 'react';
import { TabStripOrientation } from '../../../../../../types/tab-strip-types';
import { getFlatEdgeButtonCornerRadii, TabStripFlatEdge } from './utils/tab-strip-flat-edge-button';
import { joinClassNames } from '../../../../../../utils/tab-strip-internals';

export interface FlatEdgeButtonProps {
	type?: 'button' | 'reset' | 'submit' | undefined;
	'aria-label': string;
	'data-target-visible'?: boolean | undefined;
	'data-animating'?: boolean | undefined;
	onClick?: MouseEventHandler<HTMLButtonElement> | undefined;
	onContextMenu?: MouseEventHandler<HTMLButtonElement> | undefined;
	icon: ReactNode;
	label: string;
	variantClass?: keyof typeof styles | undefined;
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
	type = 'button',
	'aria-label': ariaLabel,
	'data-target-visible': targetVisible,
	'data-animating': animating,
	onClick,
	onContextMenu,
	variantClass,
}: FlatEdgeButtonProps): ReactNode {
	const radii = getFlatEdgeButtonCornerRadii(flatEdge, flatEdgeFactor);
	return (
		<button
			type={type}
			aria-label={ariaLabel}
			data-target-visible={targetVisible || undefined}
			data-animating={animating || undefined}
			onClick={onClick}
			onContextMenu={onContextMenu}
			className={joinClassNames(
				styles['ct-flat-edge-button'],
				variantClass === undefined ? undefined : styles[variantClass],
			)}
			style={{
				'--ob-comp-tab-strip-corner-top-left': `${radii.topLeft}px`,
				'--ob-comp-tab-strip-corner-top-right': `${radii.topRight}px`,
				'--ob-comp-tab-strip-corner-bottom-right': `${radii.bottomRight}px`,
				'--ob-comp-tab-strip-corner-bottom-left': `${radii.bottomLeft}px`,
			}}
			data-flat-edge={flatEdge}
			data-expansion-orientation={expansionOrientation}
		>
			<span className={styles['ct-flat-edge-button__icon']} aria-hidden='true'>
				{icon}
			</span>
			{showLabel ? (
				<span className={styles['ct-flat-edge-button__label']} data-flat-edge-label=''>
					{label}
				</span>
			) : null}
		</button>
	);
}
