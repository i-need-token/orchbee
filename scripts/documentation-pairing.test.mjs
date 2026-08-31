import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	renameSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';

import {
	collectDocumentationPairingDiagnostics,
	confirmDocumentationPairs,
	contentSha256,
	documentationStructure,
	renderPairingManifest,
	stripArgumentSeparator,
} from './documentation-pairing.mjs';

const ENGLISH = '# Standard\n\nEnglish requirement.\n';
const CHINESE = '# 规范\n\n中文要求。\n';

function withFixture(run) {
	const repositoryRoot = mkdtempSync(join(tmpdir(), 'orchbee-documentation-pairing-'));
	try {
		execFileSync('git', ['init', '--quiet'], { cwd: repositoryRoot });
		mkdirSync(join(repositoryRoot, 'docs', 'zh-CN'), { recursive: true });
		mkdirSync(join(repositoryRoot, 'scripts'), { recursive: true });
		writeFileSync(
			join(repositoryRoot, 'scripts', 'documentation-pairing.json'),
			renderPairingManifest({ formatVersion: 1, pairs: {} }),
		);
		run(repositoryRoot);
	} finally {
		rmSync(repositoryRoot, { force: true, recursive: true });
	}
}

function writeRepositoryFile(repositoryRoot, path, content) {
	const absolutePath = join(repositoryRoot, path);
	mkdirSync(dirname(absolutePath), { recursive: true });
	writeFileSync(absolutePath, content);
}

function addPair(repositoryRoot, sourcePath, english = ENGLISH, chinese = CHINESE) {
	writeRepositoryFile(repositoryRoot, sourcePath, english);
	const localizedPath = sourcePath.startsWith('docs/')
		? `docs/zh-CN/${sourcePath.slice('docs/'.length)}`
		: `${sourcePath.slice(0, -'.md'.length)}.zh-CN.md`;
	writeRepositoryFile(repositoryRoot, localizedPath, chinese);
}

function manifestPath(repositoryRoot) {
	return join(repositoryRoot, 'scripts', 'documentation-pairing.json');
}

function confirm(repositoryRoot, inputs) {
	return confirmDocumentationPairs({
		inputs,
		manifestPath: manifestPath(repositoryRoot),
		repositoryRoot,
	});
}

function diagnostics(repositoryRoot) {
	return collectDocumentationPairingDiagnostics({
		manifestPath: manifestPath(repositoryRoot),
		repositoryRoot,
	});
}

function readManifest(repositoryRoot) {
	return JSON.parse(readFileSync(manifestPath(repositoryRoot), 'utf8'));
}

test('confirmed docs mirrors and locale-suffix pairs pass', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		addPair(repositoryRoot, 'packages/example/README.md');
		confirm(repositoryRoot, ['docs/zh-CN/STANDARD.md', 'packages/example/README.zh-CN.md']);

		assert.deepEqual(diagnostics(repositoryRoot), []);
		assert.match(
			readManifest(repositoryRoot).pairs['docs/STANDARD.md'].englishSha256,
			/^[a-f0-9]{64}$/u,
		);
	});
});

test('untracked Markdown is checked while ignored dependency Markdown is excluded', () => {
	withFixture((repositoryRoot) => {
		writeRepositoryFile(repositoryRoot, '.gitignore', 'node_modules/\n');
		writeRepositoryFile(repositoryRoot, 'README.md', ENGLISH);
		writeRepositoryFile(repositoryRoot, 'node_modules/dependency/README.md', ENGLISH);

		const output = diagnostics(repositoryRoot).join('\n');
		assert.match(output, /README\.md: missing localized counterpart README\.zh-CN\.md/u);
		assert.doesNotMatch(output, /node_modules/u);
	});
});

test('docs locale-suffix files cannot substitute for the zh-CN mirror path', () => {
	withFixture((repositoryRoot) => {
		writeRepositoryFile(repositoryRoot, 'docs/STANDARD.md', ENGLISH);
		writeRepositoryFile(repositoryRoot, 'docs/STANDARD.zh-CN.md', CHINESE);

		const output = diagnostics(repositoryRoot).join('\n');
		assert.match(
			output,
			/docs\/STANDARD\.zh-CN\.md: Simplified Chinese Markdown for docs\/STANDARD\.md must use docs\/zh-CN\/STANDARD\.md/u,
		);
		assert.match(output, /missing localized counterpart docs\/zh-CN\/STANDARD\.md/u);
	});
});

test('paired Markdown symlink aliases are checked', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'CLAUDE.md');
		symlinkSync('CLAUDE.md', join(repositoryRoot, 'AGENTS.md'));
		symlinkSync('CLAUDE.zh-CN.md', join(repositoryRoot, 'AGENTS.zh-CN.md'));
		confirm(repositoryRoot, ['CLAUDE.md', 'AGENTS.md']);

		assert.deepEqual(diagnostics(repositoryRoot), []);
	});
});

test('same-structure English edits invalidate their pairing receipt', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		confirm(repositoryRoot, ['docs/STANDARD.md']);
		writeRepositoryFile(repositoryRoot, 'docs/STANDARD.md', '# Standard\n\nRevised English.\n');

		assert.match(diagnostics(repositoryRoot).join('\n'), /English content changed/u);
	});
});

test('same-structure Simplified Chinese edits invalidate their pairing receipt', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		confirm(repositoryRoot, ['docs/STANDARD.md']);
		writeRepositoryFile(repositoryRoot, 'docs/zh-CN/STANDARD.md', '# 规范\n\n修改后的中文。\n');

		assert.match(diagnostics(repositoryRoot).join('\n'), /Simplified Chinese content changed/u);
	});
});

test('coverage diagnostics report missing counterparts and missing or extra receipts', () => {
	withFixture((repositoryRoot) => {
		writeRepositoryFile(repositoryRoot, 'docs/UNPAIRED.md', ENGLISH);
		writeRepositoryFile(repositoryRoot, 'docs/zh-CN/LOCALIZED-ONLY.md', CHINESE);
		const extraReceipt = {
			formatVersion: 1,
			pairs: {
				'docs/REMOVED.md': {
					englishSha256: contentSha256(ENGLISH),
					zhCnSha256: contentSha256(CHINESE),
				},
			},
		};
		writeFileSync(manifestPath(repositoryRoot), renderPairingManifest(extraReceipt));

		const output = diagnostics(repositoryRoot).join('\n');
		assert.match(output, /missing localized counterpart/u);
		assert.match(output, /missing documentation pairing receipt/u);
		assert.match(output, /has no English Markdown source/u);
		assert.match(output, /receipt has no English Markdown source/u);
	});
});

test('manifest diagnostics reject malformed JSON, malformed receipts, and noncanonical content', () => {
	withFixture((repositoryRoot) => {
		writeFileSync(manifestPath(repositoryRoot), '{');
		assert.match(diagnostics(repositoryRoot).join('\n'), /not valid JSON/u);

		writeFileSync(manifestPath(repositoryRoot), '{"pairs":{},"formatVersion":1}\n');
		assert.match(diagnostics(repositoryRoot).join('\n'), /not canonical/u);

		writeFileSync(
			manifestPath(repositoryRoot),
			renderPairingManifest({
				formatVersion: 1,
				pairs: {
					'docs/STANDARD.md': {
						englishSha256: 'invalid',
						zhCnSha256: contentSha256(CHINESE),
					},
				},
			}),
		);
		assert.match(diagnostics(repositoryRoot).join('\n'), /lowercase SHA-256 digest/u);
	});
});

test('confirmation updates only named pairs and leaves unrelated drift visible', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/FIRST.md');
		addPair(repositoryRoot, 'docs/SECOND.md');
		confirm(repositoryRoot, ['docs/FIRST.md', 'docs/SECOND.md']);
		const secondReceipt = readManifest(repositoryRoot).pairs['docs/SECOND.md'];
		writeRepositoryFile(repositoryRoot, 'docs/FIRST.md', '# Standard\n\nFirst revision.\n');
		writeRepositoryFile(repositoryRoot, 'docs/SECOND.md', '# Standard\n\nSecond revision.\n');

		confirm(repositoryRoot, ['docs/FIRST.md']);

		assert.deepEqual(readManifest(repositoryRoot).pairs['docs/SECOND.md'], secondReceipt);
		assert.deepEqual(diagnostics(repositoryRoot), [
			'docs/SECOND.md: English content changed after pairing confirmation; review the pair, then run pnpm docs:confirm-pairing -- docs/SECOND.md',
		]);
	});
});

test('confirmation refuses structural drift without rewriting the manifest', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md', '# Standard\n\n- Requirement.\n', CHINESE);
		const before = readFileSync(manifestPath(repositoryRoot), 'utf8');

		assert.throws(
			() => confirm(repositoryRoot, ['docs/STANDARD.md']),
			/English and Simplified Chinese structures differ/u,
		);
		assert.equal(readFileSync(manifestPath(repositoryRoot), 'utf8'), before);
	});
});

test('confirmation refuses table column drift with the same row count', () => {
	withFixture((repositoryRoot) => {
		addPair(
			repositoryRoot,
			'docs/STANDARD.md',
			'# Standard\n\n| Requirement | Owner |\n| --- | --- |\n| Stable | Team |\n',
			'# 规范\n\n| 要求 | 负责人 | 状态 |\n| --- | --- | --- |\n| 稳定 | 团队 | 已完成 |\n',
		);
		const before = readFileSync(manifestPath(repositoryRoot), 'utf8');

		assert.throws(
			() => confirm(repositoryRoot, ['docs/STANDARD.md']),
			/English and Simplified Chinese structures differ/u,
		);
		assert.equal(readFileSync(manifestPath(repositoryRoot), 'utf8'), before);
	});
});

test('confirmation removes a receipt for an explicitly named paired deletion', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		execFileSync('git', ['add', 'docs/STANDARD.md', 'docs/zh-CN/STANDARD.md'], {
			cwd: repositoryRoot,
		});
		confirm(repositoryRoot, ['docs/STANDARD.md']);
		rmSync(join(repositoryRoot, 'docs', 'STANDARD.md'));
		rmSync(join(repositoryRoot, 'docs', 'zh-CN', 'STANDARD.md'));

		assert.deepEqual(confirm(repositoryRoot, ['docs/STANDARD.md']), [
			{ sourcePath: 'docs/STANDARD.md', status: 'removed' },
		]);
		assert.deepEqual(readManifest(repositoryRoot).pairs, {});
		assert.deepEqual(diagnostics(repositoryRoot), []);
	});
});

test('confirmation treats a case-only rename as deletion plus addition', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		confirm(repositoryRoot, ['docs/STANDARD.md']);
		renameSync(
			join(repositoryRoot, 'docs', 'STANDARD.md'),
			join(repositoryRoot, 'docs', 'STANDARD.tmp'),
		);
		renameSync(
			join(repositoryRoot, 'docs', 'STANDARD.tmp'),
			join(repositoryRoot, 'docs', 'Standard.md'),
		);
		renameSync(
			join(repositoryRoot, 'docs', 'zh-CN', 'STANDARD.md'),
			join(repositoryRoot, 'docs', 'zh-CN', 'STANDARD.tmp'),
		);
		renameSync(
			join(repositoryRoot, 'docs', 'zh-CN', 'STANDARD.tmp'),
			join(repositoryRoot, 'docs', 'zh-CN', 'Standard.md'),
		);

		assert.deepEqual(confirm(repositoryRoot, ['docs/STANDARD.md', 'docs/Standard.md']), [
			{ sourcePath: 'docs/STANDARD.md', status: 'removed' },
			{ sourcePath: 'docs/Standard.md', status: 'confirmed' },
		]);
		assert.deepEqual(Object.keys(readManifest(repositoryRoot).pairs), ['docs/Standard.md']);
	});
});

test('confirmation refuses a one-sided deletion', () => {
	withFixture((repositoryRoot) => {
		addPair(repositoryRoot, 'docs/STANDARD.md');
		confirm(repositoryRoot, ['docs/STANDARD.md']);
		rmSync(join(repositoryRoot, 'docs', 'zh-CN', 'STANDARD.md'));

		assert.throws(
			() => confirm(repositoryRoot, ['docs/STANDARD.md']),
			/cannot confirm while docs\/zh-CN\/STANDARD.md is missing/u,
		);
	});
});

test('content hashes canonicalize CRLF and lone carriage returns to LF', () => {
	assert.equal(contentSha256('first\r\nsecond\rthird\n'), contentSha256('first\nsecond\nthird\n'));
	assert.equal(contentSha256(Buffer.from('first\r\nsecond\n')), contentSha256('first\nsecond\n'));
});

test('inline-code structure ignores language-specific line wrapping', () => {
	assert.deepEqual(
		documentationStructure('The `same technical\nidentifier` remains.\n'),
		documentationStructure('同一个 `same technical identifier` 保持不变。\n'),
	);
});

test('command parsing accepts the pnpm argument separator', () => {
	assert.deepEqual(stripArgumentSeparator(['--', 'docs/STANDARD.md']), ['docs/STANDARD.md']);
	assert.deepEqual(stripArgumentSeparator(['docs/STANDARD.md']), ['docs/STANDARD.md']);
});
