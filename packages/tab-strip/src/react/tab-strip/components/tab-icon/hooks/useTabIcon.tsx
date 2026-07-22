import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { TabData } from '../../../../../model';
import { TAB_ICON_CRASH_ANIMATION_MS, getTabIconVisualState } from '../../../../../tab';
import { TabStripIcon } from '../../tab-strip-icon/TabStripIcon';
interface UseTabIconOptions {
	tab: Readonly<TabData>;
	availableWidth?: number;
	discardRingTreatmentEnabled?: boolean;
}
export function useTabIcon({
	tab,
	availableWidth,
	discardRingTreatmentEnabled = true,
}: UseTabIconOptions): ReactNode {
	const visual = getTabIconVisualState(tab, availableWidth, discardRingTreatmentEnabled);
	const previousCrashed = useRef(Boolean(tab.crashed));
	const mounted = useRef(false);
	const [crashTransition, setCrashTransition] = useState(false);
	useEffect(() => {
		const crashed = Boolean(tab.crashed);
		const shouldAnimate = mounted.current && crashed && !previousCrashed.current;
		mounted.current = true;
		previousCrashed.current = crashed;
		if (!shouldAnimate) {
			if (!crashed) setCrashTransition(false);
			return;
		}
		setCrashTransition(true);
		const timer = setTimeout(() => setCrashTransition(false), TAB_ICON_CRASH_ANIMATION_MS);
		return () => clearTimeout(timer);
	}, [tab.crashed]);
	const favicon = tab.faviconUrl ? (
		<img className='ct-tab-icon__favicon' src={tab.faviconUrl} alt='' draggable={false} />
	) : (
		<TabStripIcon name='default-favicon' className='ct-tab-icon__favicon' />
	);
	return (
		<span
			className='ct-tab-icon'
			aria-hidden='true'
			data-network-state={visual.networkState}
			data-loading={visual.showingLoadingAnimation || undefined}
			data-show-favicon={visual.showingFavicon || undefined}
			data-custom-favicon={visual.hasCustomFavicon || undefined}
			data-crashed={visual.showingCrashedFavicon || undefined}
			data-crash-transition={crashTransition || undefined}
			data-attention={visual.showingAttentionIndicator || undefined}
			data-discarded={visual.showingDiscardIndicator || undefined}
			data-expanded-discard-ring={visual.increasedDiscardIndicatorRadius > 0 || undefined}
		>
			<span className='ct-tab-icon__normal'>{favicon}</span>
			<TabStripIcon name='sad-tab' className='ct-tab-icon__crashed' />
			{visual.showingLoadingAnimation ? (
				<TabStripIcon name='spinner' className='ct-tab-icon__spinner' />
			) : null}
			{visual.showingDiscardIndicator ? <span className='ct-tab-icon__discard-ring' /> : null}
			{visual.showingAttentionIndicator ? <span className='ct-tab-icon__attention' /> : null}
		</span>
	);
}
