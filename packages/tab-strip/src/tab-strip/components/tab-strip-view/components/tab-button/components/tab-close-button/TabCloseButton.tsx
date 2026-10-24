import { ProductIcon } from '@orchbee/ui/components';
import styles from './TabCloseButton.module.css';
import type { MouseEvent, ReactNode } from 'react';

import { useTabCloseButton } from './hooks/useTabCloseButton';
import { useTabStripMessages } from '../../../../../../hooks/useTabStripMessages';

interface TabCloseButtonProps {
	tabTitle: string;
	touchUi?: boolean;
	onClose: (event: MouseEvent<HTMLButtonElement>, pointerType: string) => void;
}

export function TabCloseButton(props: TabCloseButtonProps): ReactNode {
	const messages = useTabStripMessages();
	const view = useTabCloseButton(props);
	return (
		<button
			type='button'
			className={styles['ct-tab__close']}
			aria-label={messages.closeTab}
			tabIndex={-1}
			onPointerDown={view.onPointerDown}
			onClick={view.onClick}
		>
			<span className={styles['ct-tab__close-target']} data-tab-close-target=''>
				<ProductIcon name='close' />
			</span>
		</button>
	);
}
