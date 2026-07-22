import styles from './TabCloseButton.module.css';
import type { MouseEvent, ReactNode } from 'react';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';

import { useTabCloseButton } from './hooks/useTabCloseButton';

export interface TabCloseButtonProps {
	tabTitle: string;
	touchUi?: boolean;
	onClose: (event: MouseEvent<HTMLButtonElement>, pointerType: string) => void;
}

export function TabCloseButton(props: TabCloseButtonProps): ReactNode {
	const view = useTabCloseButton(props);
	return (
		<button
			type='button'
			className={styles['ct-tab__close']}
			aria-label='Close'
			tabIndex={-1}
			onPointerDown={view.onPointerDown}
			onClick={view.onClick}
		>
			<span className={styles['ct-tab__close-target']}>
				<TabStripIcon name='close' />
			</span>
		</button>
	);
}
