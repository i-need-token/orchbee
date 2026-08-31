import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { assertPinnedGhosttyCheckout, readPinnedGhosttyCommit } from './ghostty-source.mjs';

function git(repositoryRoot, arguments_) {
	return execFileSync('git', ['-C', repositoryRoot, ...arguments_], {
		encoding: 'utf8',
	}).trim();
}

function withFixture(run) {
	const fixtureRoot = mkdtempSync(join(tmpdir(), 'orchbee-ghostty-source-'));
	const repositoryRoot = join(fixtureRoot, 'orchbee');
	const ghosttyRoot = join(fixtureRoot, 'ghostty');
	try {
		mkdirSync(join(repositoryRoot, 'apps', 'terminal-host'), { recursive: true });
		mkdirSync(ghosttyRoot, { recursive: true });
		git(ghosttyRoot, ['init', '--quiet']);
		writeFileSync(join(ghosttyRoot, 'source.txt'), 'pinned\n');
		git(ghosttyRoot, ['add', 'source.txt']);
		git(ghosttyRoot, [
			'-c',
			'user.name=Orchbee Test',
			'-c',
			'user.email=orchbee@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'fixture',
		]);
		const commit = git(ghosttyRoot, ['rev-parse', 'HEAD']);
		writeFileSync(
			join(repositoryRoot, 'apps', 'terminal-host', 'build.zig.zon'),
			`.{ .dependencies = .{ .ghostty = .{ .url = "https://github.com/ghostty-org/ghostty/archive/${commit}.tar.gz" } } }\n`,
		);
		run({ commit, ghosttyRoot, repositoryRoot });
	} finally {
		rmSync(fixtureRoot, { force: true, recursive: true });
	}
}

test('accepts one clean checkout at the Terminal Host Ghostty pin', () => {
	withFixture(({ commit, ghosttyRoot, repositoryRoot }) => {
		assert.equal(readPinnedGhosttyCommit(repositoryRoot), commit);
		assert.equal(assertPinnedGhosttyCheckout({ ghosttyRoot, repositoryRoot }), commit);
	});
});

test('rejects a checkout at a different commit', () => {
	withFixture(({ ghosttyRoot, repositoryRoot }) => {
		writeFileSync(join(ghosttyRoot, 'source.txt'), 'next\n');
		git(ghosttyRoot, ['add', 'source.txt']);
		git(ghosttyRoot, [
			'-c',
			'user.name=Orchbee Test',
			'-c',
			'user.email=orchbee@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'next',
		]);

		assert.throws(
			() => assertPinnedGhosttyCheckout({ ghosttyRoot, repositoryRoot }),
			/does not match the pinned commit/u,
		);
	});
});

test('rejects tracked and untracked source changes', () => {
	withFixture(({ ghosttyRoot, repositoryRoot }) => {
		writeFileSync(join(ghosttyRoot, 'source.txt'), 'dirty\n');
		writeFileSync(join(ghosttyRoot, 'untracked.txt'), 'dirty\n');

		assert.throws(
			() => assertPinnedGhosttyCheckout({ ghosttyRoot, repositoryRoot }),
			/Ghostty source must be clean/u,
		);
	});
});

test('rejects a path nested below the checkout root', () => {
	withFixture(({ ghosttyRoot, repositoryRoot }) => {
		const nestedRoot = join(ghosttyRoot, 'nested');
		mkdirSync(nestedRoot);

		assert.throws(
			() => assertPinnedGhosttyCheckout({ ghosttyRoot: nestedRoot, repositoryRoot }),
			/GHOSTTY_SOURCE must name the Ghostty Git checkout root/u,
		);
	});
});
