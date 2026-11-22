import styles from './TabStripControlButton.module.css';
import type { MouseEventHandler, PointerEventHandler, ReactNode } from 'react';
import { joinClassNames } from '../../../../../../utils/tab-strip-internals';

interface TabStripControlButtonProps {
	type?: 'button' | 'reset' | 'submit';
	'aria-label': string;
	/** A button that opens a menu names it and says whether it is open. */
	'aria-expanded'?: boolean;
	'aria-haspopup'?: 'menu';
	disabled?: boolean;
	tabIndex?: number;
	onClick?: MouseEventHandler<HTMLButtonElement>;
	onContextMenu?: MouseEventHandler<HTMLButtonElement>;
	onPointerDown?: PointerEventHandler<HTMLButtonElement>;
	icon?: ReactNode;
	label?: ReactNode;
	variantClass?: keyof typeof styles;
	inlineStart?: number;
	frameCondensed?: boolean;
}

export function TabStripControlButton({
	icon,
	label,
	frameCondensed = false,
	type = 'button',
	'aria-label': ariaLabel,
	'aria-expanded': ariaExpanded,
	'aria-haspopup': ariaHasPopup,
	disabled,
	tabIndex = -1,
	onClick,
	onContextMenu,
	onPointerDown,
	variantClass,
	inlineStart,
}: TabStripControlButtonProps): ReactNode {
	return (
		<button
			type={type}
			aria-label={ariaLabel}
			aria-expanded={ariaExpanded}
			aria-haspopup={ariaHasPopup}
			disabled={disabled}
			tabIndex={tabIndex}
			onClick={onClick}
			onContextMenu={onContextMenu}
			onPointerDown={onPointerDown}
			className={joinClassNames(
				styles['ct-tab-strip-control-button'],
				variantClass === undefined ? undefined : styles[variantClass],
			)}
			data-frame-condensed={frameCondensed || undefined}
			data-tab-strip-new-tab={variantClass === 'ct-new-tab' || undefined}
			style={{
				'--ob-comp-tab-strip-inline-start':
					inlineStart === undefined ? undefined : `${inlineStart}px`,
			}}
		>
			<span
				className={styles['ct-tab-strip-control-button__target']}
				data-tab-strip-control-target=''
			>
				{icon ? (
					<span className={styles['ct-tab-strip-control-button__icon']} aria-hidden='true'>
						{icon}
					</span>
				) : null}
				{label}
			</span>
		</button>
	);
}
