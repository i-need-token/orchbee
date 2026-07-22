import type { ReactNode } from 'react';
import type { TabData } from '../../../../model';

import { useTabIcon } from './hooks/useTabIcon';

export interface TabIconProps {
	tab: Readonly<TabData>;
	availableWidth?: number;
	discardRingTreatmentEnabled?: boolean;
}

export function TabIcon(props: TabIconProps): ReactNode {
	return useTabIcon(props);
}
