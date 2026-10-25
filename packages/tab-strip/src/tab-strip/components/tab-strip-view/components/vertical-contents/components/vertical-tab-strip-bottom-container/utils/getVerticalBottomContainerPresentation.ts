import {
	TabStripOrientation,
	VerticalTabStripCollapseState,
} from '../../../../../../../types/tab-strip-types';

import { LAYOUT_CONSTANTS } from '../../../../../../../layout/layout-constants';

interface VerticalBottomContainerPresentation {
	collapsedStyling: boolean;
	orientation: TabStripOrientation;
	crossAxisAlignment: 'start' | 'stretch';
	maximumFlexSize: 'unbounded' | 'preferred';
	insets: Readonly<{ top: number; right: number; bottom: number; left: number }>;
	buttonSize: number;
	iconSize: number;
}

/** Direct port of VerticalTabStripBottomContainer::UpdateButtonStyles. */
export function getVerticalBottomContainerPresentation(
	state: VerticalTabStripCollapseState,
): VerticalBottomContainerPresentation {
	const collapsed = state !== VerticalTabStripCollapseState.Expanded;
	const horizontalInset = collapsed ? 6 : 14;
	return {
		collapsedStyling: collapsed,
		orientation: collapsed ? TabStripOrientation.Vertical : TabStripOrientation.Horizontal,
		crossAxisAlignment: collapsed ? 'stretch' : 'start',
		maximumFlexSize: collapsed ? 'preferred' : 'unbounded',
		insets: { top: 5, right: horizontalInset, bottom: 5, left: horizontalInset },
		buttonSize: LAYOUT_CONSTANTS.verticalTabStripNewTabButtonSize,
		iconSize: LAYOUT_CONSTANTS.verticalTabStripButtonIconSize,
	};
}
