import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ManageSharingRow } from '../../manage-sharing-row/ManageSharingRow';
import { RecentActivityBubble } from '../RecentActivityBubble';

afterEach(cleanup);

const activity = Array.from({ length: 7 }, (_, index) => ({
	id: `group-${index}`,
	scope: 'group' as const,
	title: `Update ${index}`,
	description: 'Alex',
	timeText: `${index}m`,
	action: index === 0 ? ('manage-sharing' as const) : ('none' as const),
}));

describe('shared group views', () => {
	it('renders the three-circle face pile with a +N overflow and accessible member count', () => {
		render(
			<ManageSharingRow
				members={Array.from({ length: 6 }, (_, index) => ({
					id: `${index}`,
					displayName: `Member ${index}`,
				}))}
			/>,
		);
		expect(screen.getByRole('menuitem', { name: 'Manage group, 6 members' })).toBeTruthy();
		expect(screen.getByText('+4')).toBeTruthy();
	});

	it('renders no more than five recent group updates', () => {
		render(<RecentActivityBubble activity={activity} />);
		expect(screen.getAllByRole('row')).toHaveLength(5);
	});

	it('runs enabled activity actions and exposes the see-all menu', () => {
		const onAction = vi.fn<() => void>();
		const onSeeAll = vi.fn<() => void>();
		render(<RecentActivityBubble activity={activity} onAction={onAction} onSeeAll={onSeeAll} />);
		fireEvent.click(screen.getByRole('row', { name: 'Update 0' }));
		expect(onAction).toHaveBeenCalledWith(activity[0]);
		fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
		fireEvent.click(screen.getByRole('menuitem', { name: 'See all activity' }));
		expect(onSeeAll).toHaveBeenCalledOnce();
	});
});
