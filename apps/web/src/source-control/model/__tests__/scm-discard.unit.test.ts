import { sourceControlMessages } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { scmDiscardQuestion, type ScmCurrentChange } from '../scm-discard';
import type { ScmResourceStatus } from '../scm-resource';

function change(path: string, status: ScmResourceStatus): ScmCurrentChange {
	return { repository: '/w', resource: { group: 'workingTree', original: null, path, status } };
}

describe('the question before discarding changes', () => {
	it('asks to discard tracked files, or to restore them when every one is deleted', () => {
		const discard = scmDiscardQuestion(
			[change('/w/a.ts', 'modified'), change('/w/b.ts', 'deleted')],
			'linux',
			'/',
		);
		expect(discard.title).toEqual({
			...sourceControlMessages.discardTrackedTitle,
			values: { count: 2, name: 'a.ts' },
		});
		expect(discard.risk).toBe('destructive');

		const restore = scmDiscardQuestion([change('/w/b.ts', 'deleted')], 'linux', '/');
		expect(restore.title).toEqual({
			...sourceControlMessages.restoreTitle,
			values: { count: 1, name: 'b.ts' },
		});
	});

	it('names the trash of the machine holding untracked files, and asks once with tracked ones', () => {
		const untracked = scmDiscardQuestion(
			[change('C:\\w\\new.ts', 'untracked'), change('C:\\w\\old.log', 'ignored')],
			'windows',
			'\\',
		);
		expect(untracked.title).toEqual({
			...sourceControlMessages.deleteUntrackedTitle,
			values: { count: 2, name: 'new.ts', os: 'windows' },
		});
		expect(untracked.confirmLabel).toEqual({
			...sourceControlMessages.deleteUntrackedConfirm,
			values: { count: 2, name: 'new.ts', os: 'windows' },
		});

		const mixed = scmDiscardQuestion(
			[change('/w/new.ts', 'untracked'), change('/w/a.ts', 'modified')],
			'macos',
			'/',
		);
		expect(mixed.title).toEqual({
			...sourceControlMessages.deleteUntrackedTitle,
			values: { count: 1, name: 'new.ts', os: 'macos' },
		});
		expect(mixed.description).toEqual({
			...sourceControlMessages.discardMixedDescription,
			values: { name: 'a.ts', os: 'macos', tracked: 1, untracked: 1 },
		});
		expect(mixed.confirmLabel).toEqual({
			...sourceControlMessages.discardTrackedConfirm,
			values: { count: 2 },
		});
	});
});
