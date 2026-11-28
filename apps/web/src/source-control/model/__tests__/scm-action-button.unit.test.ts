import { describe, expect, it } from 'vitest';

import type { GitRepositorySummary } from '../../../file-session';
import {
	scmActionButton,
	type ScmActionButton,
	type ScmActionButtonInput,
} from '../scm-action-button';

type Summary = Omit<Partial<GitRepositorySummary>, 'counts' | 'head'> & {
	readonly counts?: Partial<GitRepositorySummary['counts']>;
	readonly head?: Partial<GitRepositorySummary['head']>;
};

function button(
	summary: Summary,
	state: Partial<Omit<ScmActionButtonInput, 'summary'>> = {},
): ScmActionButton | null {
	return scmActionButton({
		committing: false,
		postCommit: false,
		remembered: null,
		remotes: [{ fetchUrl: 'u', name: 'origin', pushUrl: 'u' }],
		smartCommit: { enabled: false, suggest: true },
		syncing: false,
		...state,
		summary: {
			busy: null,
			kind: 'root',
			operation: 'none',
			repository: '/r',
			revision: 1,
			stale: false,
			truncated: false,
			upstream: { ahead: 0, behind: 0, name: 'origin/main' },
			warm: true,
			...summary,
			counts: { index: 0, merge: 0, untracked: 0, workingTree: 0, ...summary.counts },
			head: {
				branch: 'main',
				commit: 'a'.repeat(40),
				detached: false,
				unborn: false,
				...summary.head,
			},
		},
	});
}

/** The button as a line: enabled or not, the primary's icon, title and tooltip, then the dropdown. */
function describeButton(shown: ScmActionButton | null): string | null {
	if (shown === null) return null;
	const { primary } = shown;
	const title =
		primary.title.kind === 'sync'
			? `sync ${primary.title.behind}↓ ${primary.title.ahead}↑`
			: primary.title.kind;
	const menu = shown.secondary.map((group) => group.map((item) => item.title.kind).join(','));
	return [
		shown.enabled ? 'enabled' : 'disabled',
		primary.icon,
		title,
		primary.tooltip.kind,
		...menu,
	]
		.join(' | ')
		.trim();
}

describe('scmActionButton', () => {
	it('shows no button before the status is read or without a HEAD', () => {
		expect(button({ warm: false, counts: { index: 1 } })).toBeNull();
		expect(button({ head: { branch: null, commit: null } })).toBeNull();
	});

	it('commits staged changes, and unstaged ones only while smart commit is enabled or suggested', () => {
		expect(describeButton(button({ counts: { index: 1 } }))).toBe(
			'enabled | check | commit | commit | commit,commit-amend | commit-and-push,commit-and-sync',
		);
		expect(button({ counts: { index: 1 } })?.primary.tooltip).toEqual({
			branch: 'main',
			kind: 'commit',
		});
		expect(describeButton(button({ counts: { untracked: 1 } }))).toMatch(
			/^enabled \| check \| commit /u,
		);
		expect(
			describeButton(
				button({ counts: { workingTree: 1 } }, { smartCommit: { enabled: false, suggest: false } }),
			),
		).toMatch(/^disabled \| check \| commit /u);
		expect(
			describeButton(
				button({ counts: { workingTree: 1 } }, { smartCommit: { enabled: true, suggest: false } }),
			),
		).toMatch(/^enabled /u);
	});

	it('publishes a branch without an upstream and syncs one that is ahead or behind', () => {
		expect(describeButton(button({ upstream: null }))).toBe(
			'enabled | cloud-upload | publish | publish',
		);
		expect(describeButton(button({ upstream: null }, { syncing: true }))).toBe(
			'disabled | syncing | publish | publishing',
		);
		expect(describeButton(button({ upstream: { ahead: 2, behind: 1, name: 'origin/main' } }))).toBe(
			'enabled | sync | sync 1↓ 2↑ | sync',
		);
		expect(
			button({ upstream: { ahead: 2, behind: 0, name: 'origin/main' } })?.primary.tooltip,
		).toEqual({ kind: 'sync', tooltip: { ahead: 2, kind: 'push', upstream: 'origin/main' } });
		expect(
			describeButton(
				button({ upstream: { ahead: 0, behind: 3, name: 'origin/main' } }, { syncing: true }),
			),
		).toBe('disabled | syncing | sync 3↓ 0↑ | syncing');
		// Changes to commit come first; an even branch falls back to a disabled Commit.
		expect(describeButton(button({ counts: { index: 1 }, upstream: null }))).toMatch(
			/^enabled \| check \| commit /u,
		);
		expect(describeButton(button({}))).toMatch(/^disabled \| check \| commit \| commit /u);
	});

	it('continues a rebase or merge once no conflict is left, without the dropdown', () => {
		expect(describeButton(button({ counts: { merge: 1 }, operation: 'rebase' }))).toBe(
			'disabled | check | continue | continue-rebase',
		);
		expect(describeButton(button({ operation: 'rebase' }))).toBe(
			'enabled | check | continue | continue-rebase',
		);
		expect(describeButton(button({ operation: 'merge' }, { committing: true }))).toBe(
			'disabled | check | continue | continuing-merge',
		);
		// A rebase or merge in progress hides Publish and Sync.
		expect(describeButton(button({ operation: 'merge', upstream: null }))).toBe(
			'enabled | check | continue | continue-merge',
		);
	});

	it('commits on a detached HEAD without the dropdown', () => {
		const detached = button({ counts: { index: 1 }, head: { branch: null, detached: true } });
		expect(describeButton(detached)).toBe('enabled | check | commit | commit');
		expect(detached?.primary.tooltip).toEqual({ branch: null, kind: 'commit' });
	});

	it("offers the remembered post-commit command first and spins it while that commit's steps run", () => {
		expect(describeButton(button({ counts: { index: 1 } }, { remembered: 'push' }))).toMatch(
			/^enabled \| arrow-up \| commit-and-push \| commit-and-push /u,
		);
		expect(
			describeButton(button({ counts: { index: 1 } }, { committing: true, remembered: 'sync' })),
		).toMatch(/^disabled \| syncing \| commit-and-sync \| committing-and-syncing /u);
		// The push after the commit still blocks Publish, Sync and another commit.
		expect(
			describeButton(button({ upstream: null }, { postCommit: true, remembered: 'push' })),
		).toMatch(/^disabled \| syncing \| commit-and-push \| committing-and-pushing /u);
	});
});
