import { useState, type CSSProperties, type PointerEvent, type ReactNode } from 'react';
import { getRecentActivitySections } from '../../../../../groups';
import type { TabGroupActivity } from '../../../../../model';
import { RecentActivityRow } from '../../recent-activity-row/RecentActivityRow';
import { TabStripIcon } from '../../tab-strip-icon/TabStripIcon';
interface UseRecentActivityBubbleOptions {
	activity: readonly Readonly<TabGroupActivity>[];
	style?: CSSProperties;
	onAction?: (item: Readonly<TabGroupActivity>) => void;
	onSeeAll?: () => void;
	onClose?: () => void;
}
export function useRecentActivityBubble({
	activity,
	style,
	onAction,
	onSeeAll,
	onClose,
}: UseRecentActivityBubbleOptions): ReactNode {
	const sections = getRecentActivitySections(activity);
	const [menuOpen, setMenuOpen] = useState(false);
	return (
		<div
			className='ct-recent-activity'
			role='dialog'
			aria-label='Recent activity'
			style={style}
			onPointerDown={(event: PointerEvent<HTMLDivElement>) => event.stopPropagation()}
		>
			<header>
				<strong>{'Recent activity'}</strong>
				<span className='ct-recent-activity__actions'>
					<button
						type='button'
						aria-label='More actions'
						onClick={() => setMenuOpen((open) => !open)}
					>
						{'\u22EE'}
					</button>

					<button type='button' aria-label='Close recent activity' onClick={onClose}>
						<TabStripIcon name='close' />
					</button>
				</span>
				{menuOpen ? (
					<span className='ct-recent-activity__menu' role='menu'>
						<button
							type='button'
							role='menuitem'
							onClick={() => {
								setMenuOpen(false);
								onSeeAll?.();
							}}
						>
							{'See all activity'}
						</button>
					</span>
				) : null}
			</header>
			{sections.empty ? <p className='ct-recent-activity__empty'>{'No updates'}</p> : null}
			{sections.tabActivity ? (
				<section>
					<h3>{'Latest tab update'}</h3>
					<div className='ct-recent-activity__rows' role='rowgroup'>
						<RecentActivityRow
							item={sections.tabActivity}
							{...(onAction === undefined
								? {}
								: {
										onAction,
									})}
						/>
					</div>
				</section>
			) : null}
			{sections.groupActivity.length > 0 ? (
				<section>
					<h3>{'Latest group updates'}</h3>
					<div className='ct-recent-activity__rows' role='rowgroup'>
						{sections.groupActivity.map((item) => (
							<RecentActivityRow
								key={item.id}
								item={item}
								{...(onAction === undefined
									? {}
									: {
											onAction,
										})}
							/>
						))}
					</div>
				</section>
			) : null}
		</div>
	);
}
