// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react';
import { createElement, type ReactNode, type TransitionEvent as ReactTransitionEvent } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import type { TabGroupData } from '../../../../../../model/tab-model';
import { useVerticalGroupCollapseCompletion } from '../useVerticalGroupCollapseCompletion';

const expandedGroup: Readonly<TabGroupData> = {
	id: 'work',
	title: 'Work',
	color: 'blue',
	collapsed: false,
};

function Harness({ groups }: { readonly groups: readonly Readonly<TabGroupData>[] }): ReactNode {
	const { fullyCollapsedGroupIds, onGroupTabsTransitionEnd } =
		useVerticalGroupCollapseCompletion(groups);
	return createElement('div', {
		'data-testid': 'group-tabs',
		'data-fully-collapsed': fullyCollapsedGroupIds.has('work') || undefined,
		onTransitionEnd: (event: ReactTransitionEvent<HTMLDivElement>) =>
			onGroupTabsTransitionEnd(event, 'work'),
	});
}

afterEach(cleanup);

describe('useVerticalGroupCollapseCompletion', () => {
	it('waits for the grid transition before marking a newly collapsed group complete', () => {
		const { getByTestId, rerender } = render(createElement(Harness, { groups: [expandedGroup] }));
		const groupTabs = getByTestId('group-tabs');

		rerender(
			createElement(Harness, {
				groups: [{ ...expandedGroup, collapsed: true }],
			}),
		);
		expect(groupTabs.hasAttribute('data-fully-collapsed')).toBe(false);

		fireEvent.transitionEnd(groupTabs, { propertyName: 'grid-template-rows' });
		expect(groupTabs.getAttribute('data-fully-collapsed')).toBe('true');

		rerender(createElement(Harness, { groups: [expandedGroup] }));
		expect(groupTabs.hasAttribute('data-fully-collapsed')).toBe(false);
	});
});
