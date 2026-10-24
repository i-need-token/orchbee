import styles from './TabStripControlButton.module.css';
import type { MouseEventHandler, ReactNode } from 'react';
import { joinClassNames } from '../../../../../../utils/tab-strip-internals';

interface TabStripControlButtonProps {
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
	frameCondensed?: boolean;
}

export function TabStripControlButton({
	icon,
	label,
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
