import { execFileSync } from 'node:child_process';
import { readFileSync, realpathSync } from 'node:fs';
import { join } from 'node:path';

const GHOSTTY_ARCHIVE_PIN =
	/https:\/\/github\.com\/ghostty-org\/ghostty\/archive\/([a-f0-9]{40})\.tar\.gz/gu;

function runGit(ghosttyRoot, arguments_) {
	try {
		return execFileSync('git', ['-C', ghosttyRoot, ...arguments_], {
			encoding: 'utf8',
			maxBuffer: 16 * 1024 * 1024,
		});
	} catch (error) {
		throw new Error(`Ghostty source must be a readable Git checkout: ${ghosttyRoot}`, {
			cause: error,
		});
	}
}

export function readPinnedGhosttyCommit(repositoryRoot) {
	const manifestPath = join(repositoryRoot, 'apps', 'terminal-host', 'build.zig.zon');
	const manifest = readFileSync(manifestPath, 'utf8');
	const matches = [...manifest.matchAll(GHOSTTY_ARCHIVE_PIN)];
	if (matches.length !== 1 || matches[0]?.[1] === undefined) {
		throw new Error(`${manifestPath} must contain exactly one commit-pinned Ghostty archive URL`);
	}
	return matches[0][1];
}

export function assertPinnedGhosttyCheckout({ ghosttyRoot, repositoryRoot }) {
	const expectedCommit = readPinnedGhosttyCommit(repositoryRoot);
	const checkoutRoot = runGit(ghosttyRoot, ['rev-parse', '--show-toplevel']).trim();
	if (realpathSync(checkoutRoot) !== realpathSync(ghosttyRoot)) {
		throw new Error(`GHOSTTY_SOURCE must name the Ghostty Git checkout root: ${ghosttyRoot}`);
	}

	const actualCommit = runGit(ghosttyRoot, ['rev-parse', 'HEAD']).trim();
	if (actualCommit !== expectedCommit) {
		throw new Error(
			`Ghostty source HEAD ${actualCommit} does not match the pinned commit ${expectedCommit}`,
		);
	}

	const status = runGit(ghosttyRoot, [
		'status',
		'--porcelain=v1',
		'--untracked-files=all',
	]).trimEnd();
	if (status !== '') {
		throw new Error(`Ghostty source must be clean at ${expectedCommit}; git status:\n${status}`);
	}
	return expectedCommit;
}
