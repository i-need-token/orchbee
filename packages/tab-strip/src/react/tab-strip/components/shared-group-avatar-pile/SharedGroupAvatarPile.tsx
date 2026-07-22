import type { ReactNode } from 'react';
import type { TabGroupMember } from '../../../../model';
import { getSharedGroupAvatarSlots } from '../../../../groups';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
export interface SharedGroupAvatarPileProps {
	members: readonly Readonly<TabGroupMember>[];
}
export function SharedGroupAvatarPile({ members }: SharedGroupAvatarPileProps): ReactNode {
	const slots = getSharedGroupAvatarSlots(members.length);
	return (
		<span className='ct-shared-avatar-pile' aria-hidden='true'>
			{slots.map((slot) => {
				if (slot.kind === 'overflow') {
					return (
						<span key='overflow' className='ct-shared-avatar ct-shared-avatar--overflow'>
							{'+'}
							{slot.hiddenMemberCount}
						</span>
					);
				}
				const member = members[slot.memberIndex];
				return (
					<span
						key={member?.id ?? slot.memberIndex}
						className='ct-shared-avatar'
						data-clipped={slot.clippedOnTrailingEdge || undefined}
						title={member?.displayName}
					>
						{member?.avatarUrl ? (
							<img src={member.avatarUrl} alt='' />
						) : (
							<TabStripIcon name='shared-group' />
						)}
					</span>
				);
			})}
		</span>
	);
}
