import { describe, expect, it } from 'vitest';
import {
	SHARED_GROUP_AVATAR_CIRCLE_SIZE,
	SHARED_GROUP_AVATAR_OVERLAP,
	getSharedGroupAvatarPileWidth,
	getSharedGroupAvatarSlots,
} from '../avatar-container';

describe('ManageSharingAvatarContainer', () => {
	it('matches the 20dp circles and 4dp overlap', () => {
		expect(SHARED_GROUP_AVATAR_CIRCLE_SIZE).toBe(20);
		expect(SHARED_GROUP_AVATAR_OVERLAP).toBe(4);
		expect(getSharedGroupAvatarPileWidth(3)).toBe(52);
	});

	it('shows each of up to three members', () => {
		expect(getSharedGroupAvatarSlots(3)).toEqual([
			{ kind: 'member', memberIndex: 0, clippedOnTrailingEdge: true },
			{ kind: 'member', memberIndex: 1, clippedOnTrailingEdge: true },
			{ kind: 'member', memberIndex: 2, clippedOnTrailingEdge: false },
		]);
	});

	it('replaces the third circle with the count beyond the first two', () => {
		expect(getSharedGroupAvatarSlots(7).at(-1)).toEqual({
			kind: 'overflow',
			hiddenMemberCount: 5,
			clippedOnTrailingEdge: false,
		});
	});
});
