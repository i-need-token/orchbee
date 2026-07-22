import type { CSSProperties, ReactNode } from 'react';
import type { TabGroupActivity } from '../../../../model';

import { useRecentActivityBubble } from './hooks/useRecentActivityBubble';

export interface RecentActivityBubbleProps {
	activity: readonly Readonly<TabGroupActivity>[];
	style?: CSSProperties;
	onAction?: (item: Readonly<TabGroupActivity>) => void;
	onSeeAll?: () => void;
	onClose?: () => void;
}

export function RecentActivityBubble(props: RecentActivityBubbleProps): ReactNode {
	return useRecentActivityBubble(props);
}
