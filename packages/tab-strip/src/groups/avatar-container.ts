export const SHARED_GROUP_AVATAR_CIRCLE_SIZE = 20;
export const SHARED_GROUP_AVATAR_ICON_SIZE = 12;
export const SHARED_GROUP_AVATAR_TEXT_SIZE = 12;
export const SHARED_GROUP_AVATAR_OVERLAP = 4;
export const SHARED_GROUP_AVATAR_MAX_VISIBLE = 3;

export type SharedGroupAvatarSlot =
	| Readonly<{ kind: 'member'; memberIndex: number; clippedOnTrailingEdge: boolean }>
	| Readonly<{ kind: 'overflow'; hiddenMemberCount: number; clippedOnTrailingEdge: false }>;

/** Ports ManageSharingAvatarContainer::RebuildChildren(). */
export function getSharedGroupAvatarSlots(memberCount: number): readonly SharedGroupAvatarSlot[] {
	const count = Math.max(0, Math.trunc(memberCount));
	if (count === 0) return [];
	const slots: SharedGroupAvatarSlot[] = [];
	slots.push({ kind: 'member', memberIndex: 0, clippedOnTrailingEdge: count > 1 });
	if (count > 1) slots.push({ kind: 'member', memberIndex: 1, clippedOnTrailingEdge: count > 2 });
	if (count === 3) slots.push({ kind: 'member', memberIndex: 2, clippedOnTrailingEdge: false });
	else if (count > 3)
		slots.push({ kind: 'overflow', hiddenMemberCount: count - 2, clippedOnTrailingEdge: false });
	return slots;
}

export function getSharedGroupAvatarPileWidth(memberCount: number): number {
	const slots = getSharedGroupAvatarSlots(memberCount).length;
	return slots === 0
		? 0
		: SHARED_GROUP_AVATAR_CIRCLE_SIZE +
				(slots - 1) * (SHARED_GROUP_AVATAR_CIRCLE_SIZE - SHARED_GROUP_AVATAR_OVERLAP);
}
