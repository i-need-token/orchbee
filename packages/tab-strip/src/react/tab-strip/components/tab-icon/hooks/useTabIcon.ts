import { useEffect, useRef, useState } from 'react';
import type { TabData } from '../../../../../model';
import {
	TAB_ICON_CRASH_ANIMATION_MS,
	getTabIconVisualState,
	type TabIconVisualState,
} from '../../../../../tab';

interface UseTabIconOptions {
	tab: Readonly<TabData>;
	availableWidth?: number;
	discardRingTreatmentEnabled?: boolean;
}

interface TabIconView {
	visual: Readonly<TabIconVisualState>;
	crashTransition: boolean;
}

export function useTabIcon({
	tab,
	availableWidth,
	discardRingTreatmentEnabled = true,
}: UseTabIconOptions): TabIconView {
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

	return { visual, crashTransition };
}
