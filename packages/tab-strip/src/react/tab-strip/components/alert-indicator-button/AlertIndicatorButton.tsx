import type { ReactNode } from 'react';
import type { TabAlertState } from '../../../../model';

import { useAlertIndicatorButton } from './hooks/useAlertIndicatorButton';

export interface AlertIndicatorButtonProps {
	alertState: TabAlertState;
	active: boolean;
	largestSelectableRegionWidth: number;
	enableTabMuting?: boolean;
	onToggleAudioMute?: (muted: boolean) => void;
}

export function AlertIndicatorButton(props: AlertIndicatorButtonProps): ReactNode {
	return useAlertIndicatorButton(props);
}
