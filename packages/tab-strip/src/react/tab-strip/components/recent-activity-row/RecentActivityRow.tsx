import type { ReactNode } from 'react';
import type { TabGroupActivity } from '../../../../model';
import { getRecentActivityDescription, isRecentActivityActionEnabled } from '../../../../groups';
import { RecentActivityImage } from '../recent-activity-image/RecentActivityImage';
export interface RecentActivityRowProps {
	item: Readonly<TabGroupActivity>;
	onAction?: (item: Readonly<TabGroupActivity>) => void;
}
export function RecentActivityRow({ item, onAction }: RecentActivityRowProps): ReactNode {
	const description = getRecentActivityDescription(item);
	return (
		<button
			type='button'
			className='ct-recent-activity__row'
			role='row'
			aria-label={item.title}
			aria-description={description}
			disabled={!isRecentActivityActionEnabled(item.action)}
			onClick={() => onAction?.(item)}
		>
			<RecentActivityImage item={item} />
			<span className='ct-recent-activity__labels'>
				<strong>{item.title}</strong>
				<span>
					<span className='ct-recent-activity__description'>{item.description}</span>
					{item.description ? ' • ' : ''}
					{item.timeText}
				</span>
			</span>
		</button>
	);
}
