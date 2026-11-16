import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { SettingsSurfaceSlotsContext } from '../../../contexts/settings-surface-slots';

/**
 * Holds the dialog's child-page slot while this page is mounted: the category hides under it, and
 * Escape runs the page's current Back. The dialog notes the control that opened the page before
 * the title takes focus, so going back returns focus there. Without a dialog the page renders where
 * it stands.
 */
export function useSettingsChildPageEntry(
	onBack: () => void,
	title: React.RefObject<HTMLElement | null>,
): HTMLElement | null {
	const slots = React.useContext(SettingsSurfaceSlotsContext);
	const back = useMemoizedFn(onBack);
	const enter = slots?.enterChildPage;
	React.useLayoutEffect(() => {
		const leave = enter?.({ back });
		title.current?.focus({ preventScroll: true });
		return leave;
	}, [back, enter, title]);
	return slots?.childPageHost ?? null;
}
