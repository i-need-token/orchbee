import type { ReactNode } from 'react';
import type { TabGroupActivity } from '../../../../model';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
export interface RecentActivityImageProps {
	item: Readonly<TabGroupActivity>;
}
export function RecentActivityImage({ item }: RecentActivityImageProps): ReactNode {
	return (
		<span className='ct-recent-activity__image' aria-hidden='true'>
			<span className='ct-recent-activity__avatar'>
				{item.avatarUrl ? (
					<img src={item.avatarUrl} alt='' />
				) : (
					<TabStripIcon name='shared-group' />
				)}
			</span>
			{item.scope === 'tab' ? (
				<span className='ct-recent-activity__favicon'>
					{item.faviconUrl ? (
						<img src={item.faviconUrl} alt='' />
					) : (
						<TabStripIcon name='default-favicon' />
					)}
				</span>
			) : null}
		</span>
	);
}
