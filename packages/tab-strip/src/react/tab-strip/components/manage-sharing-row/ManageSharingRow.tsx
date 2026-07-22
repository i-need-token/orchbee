import type { ReactNode } from 'react';
import type { TabGroupMember } from '../../../../model';
import { SharedGroupAvatarPile } from '../shared-group-avatar-pile/SharedGroupAvatarPile';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
export interface ManageSharingRowProps {
	members: readonly Readonly<TabGroupMember>[];
	onClick?: () => void;
}
export function ManageSharingRow({ members, onClick }: ManageSharingRowProps): ReactNode {
	return (
		<button
			type='button'
			className='ct-manage-sharing-row'
			role='menuitem'
			aria-label={`Manage group, ${members.length} ${members.length === 1 ? 'member' : 'members'}`}
			onClick={onClick}
		>
			<TabStripIcon name='shared-group' className='ct-manage-sharing-row__icon' />
			<span>{'Manage group'}</span>
			<SharedGroupAvatarPile members={members} />
		</button>
	);
}
