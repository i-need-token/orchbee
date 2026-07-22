import styles from './TabStripControlButton.module.css';
import type { MouseEventHandler, ReactNode } from 'react';
import { getTabStripControlButtonRadii, TabStripControlButtonEdge } from '../../../shared';
import { joinClassNames } from '../../utils/tab-strip-internals';

export interface TabStripControlButtonProps {
	type?: 'button' | 'reset' | 'submit';
	'aria-label': string;
	disabled?: boolean;
	tabIndex?: number;
	onClick?: MouseEventHandler<HTMLButtonElement>;
	onContextMenu?: MouseEventHandler<HTMLButtonElement>;
	icon?: ReactNode;
	label?: ReactNode;
	variantClass?: keyof typeof styles;
	inlineStart?: number;
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
	type = 'button',
	'aria-label': ariaLabel,
	disabled,
	tabIndex = -1,
	onClick,
	onContextMenu,
	variantClass,
	inlineStart,
}: TabStripControlButtonProps): ReactNode {
	const radii = getTabStripControlButtonRadii({
		fixedFlatEdge,
		animatedFlatEdge,
		flatEdgeFactor,
		frameCondensed,
		...(leftCornerRadius === undefined ? {} : { leftCornerRadius }),
		...(rightCornerRadius === undefined ? {} : { rightCornerRadius }),
		...(flatCornerRadius === undefined ? {} : { flatCornerRadius }),
	});
	return (
		<button
			type={type}
			aria-label={ariaLabel}
			disabled={disabled}
			tabIndex={tabIndex}
			onClick={onClick}
			onContextMenu={onContextMenu}
			className={joinClassNames(
				styles['ct-tab-strip-control-button'],
				variantClass === undefined ? undefined : styles[variantClass],
			)}
			data-frame-condensed={frameCondensed || undefined}
			data-fixed-flat-edge={fixedFlatEdge}
			data-animated-flat-edge={animatedFlatEdge}
			style={{
				'--ob-comp-tab-strip-inline-start': `${inlineStart}px`,
				'--ob-comp-tab-strip-corner-top-left': `${radii.topLeft}px`,
				'--ob-comp-tab-strip-corner-top-right': `${radii.topRight}px`,
				'--ob-comp-tab-strip-corner-bottom-right': `${radii.bottomRight}px`,
				'--ob-comp-tab-strip-corner-bottom-left': `${radii.bottomLeft}px`,
			}}
		>
			{icon ? (
				<span className={styles['ct-tab-strip-control-button__icon']} aria-hidden='true'>
					{icon}
				</span>
			) : null}
			{label}
		</button>
	);
}
