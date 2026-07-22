import type { MouseEvent, ReactNode } from 'react';

import { useTabCloseButton } from './hooks/useTabCloseButton';

export interface TabCloseButtonProps {
	tabTitle: string;
	touchUi?: boolean;
	onClose: (event: MouseEvent<HTMLButtonElement>, pointerType: string) => void;
}

export function TabCloseButton(props: TabCloseButtonProps): ReactNode {
	return useTabCloseButton(props);
}
