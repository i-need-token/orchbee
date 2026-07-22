import type { ReactNode } from 'react';
import type { TabData } from '../../../../model';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';

import { useTabIcon } from './hooks/useTabIcon';

export interface TabIconProps {
	tab: Readonly<TabData>;
	availableWidth?: number;
	discardRingTreatmentEnabled?: boolean;
}

export function TabIcon(props: TabIconProps): ReactNode {
	const view = useTabIcon(props);
	const favicon = props.tab.faviconUrl ? (
		<img className='ct-tab-icon__favicon' src={props.tab.faviconUrl} alt='' draggable={false} />
	) : (
		<TabStripIcon name='default-favicon' className='ct-tab-icon__favicon' />
	);

	return (
		<span
			className='ct-tab-icon'
			aria-hidden='true'
			data-network-state={view.visual.networkState}
			data-loading={view.visual.showingLoadingAnimation || undefined}
			data-show-favicon={view.visual.showingFavicon || undefined}
			data-custom-favicon={view.visual.hasCustomFavicon || undefined}
			data-crashed={view.visual.showingCrashedFavicon || undefined}
			data-crash-transition={view.crashTransition || undefined}
			data-attention={view.visual.showingAttentionIndicator || undefined}
			data-discarded={view.visual.showingDiscardIndicator || undefined}
			data-expanded-discard-ring={view.visual.increasedDiscardIndicatorRadius > 0 || undefined}
		>
			<span className='ct-tab-icon__normal'>{favicon}</span>
			<TabStripIcon name='sad-tab' className='ct-tab-icon__crashed' />
			{view.visual.showingLoadingAnimation ? (
				<TabStripIcon name='spinner' className='ct-tab-icon__spinner' />
			) : null}
			{view.visual.showingDiscardIndicator ? <span className='ct-tab-icon__discard-ring' /> : null}
			{view.visual.showingAttentionIndicator ? <span className='ct-tab-icon__attention' /> : null}
		</span>
	);
}
