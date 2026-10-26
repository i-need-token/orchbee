import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const REPOSITORY_ROOT = resolve(import.meta.dirname, '..');
const LOCALIZED_MARKDOWN_SUFFIX = '.zh-CN.md';
const RECEIPT_LINE =
	/^<!-- documentation-pairing: english-sha256=([a-f0-9]{64}) zh-cn-sha256=([a-f0-9]{64}) -->$/u;

function errorMessage(error) {
	return error instanceof Error ? error.message : String(error);
}

function normalizeNewlines(content) {
	const text = typeof content === 'string' ? content : content.toString('utf8');
	return text.replaceAll(/\r\n?/gu, '\n');
}

function comparePaths(left, right) {
	if (left < right) return -1;
	if (left > right) return 1;
	return 0;
}

function repositoryPathEntryExists(repositoryRoot, repositoryRelativePath) {
	let parentPath = repositoryRoot;
	for (const segment of repositoryRelativePath.split('/')) {
		let entries;
		try {
			entries = readdirSync(parentPath);
		} catch (error) {
			if (
				error instanceof Error &&
				'code' in error &&
				(error.code === 'ENOENT' || error.code === 'ENOTDIR')
			) {
				return false;
			}
			throw error;
		}
		if (!entries.includes(segment)) return false;
		parentPath = join(parentPath, segment);
	}
	return true;
}

function discoverRepositoryMarkdownPaths(repositoryRoot) {
	const output = execFileSync(
		'git',
		['ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', '*.md'],
		{
			cwd: repositoryRoot,
			encoding: 'utf8',
			maxBuffer: 16 * 1024 * 1024,
		},
	);
	return [...new Set(output.split('\0').filter((path) => path.length > 0))]
		.filter((path) => repositoryPathEntryExists(repositoryRoot, path))
		.toSorted();
}

function assertSourcePath(sourcePath) {
	const segments = sourcePath.split('/');
	if (
		!sourcePath.endsWith('.md') ||
		sourcePath.endsWith(LOCALIZED_MARKDOWN_SUFFIX) ||
		sourcePath.includes('\\') ||
		(segments[0] === 'docs' && segments[1] === 'zh-CN') ||
		segments.some((segment) => segment === '' || segment === '.' || segment === '..')
	) {
		throw new Error(`Invalid English Markdown source path "${sourcePath}"`);
	}
}

function sourcePathForLocalizedPath(localizedPath) {
	if (localizedPath.startsWith('docs/zh-CN/')) {
		const sourcePath = `docs/${localizedPath.slice('docs/zh-CN/'.length)}`;
		assertSourcePath(sourcePath);
		return sourcePath;
	}
	if (!localizedPath.endsWith(LOCALIZED_MARKDOWN_SUFFIX)) {
		throw new Error(`Invalid Simplified Chinese Markdown path "${localizedPath}"`);
	}
	const sourcePath = `${localizedPath.slice(0, -LOCALIZED_MARKDOWN_SUFFIX.length)}.md`;
	assertSourcePath(sourcePath);
	return sourcePath;
}

function sourcePathFromInput(repositoryRoot, input) {
	if (typeof input !== 'string' || input.length === 0) {
		throw new Error('Documentation pairing paths must be non-empty strings');
	}
	const absolutePath = isAbsolute(input) ? resolve(input) : resolve(repositoryRoot, input);
	const relativePath = relative(repositoryRoot, absolutePath);
	if (
		relativePath === '' ||
		relativePath === '..' ||
		relativePath.startsWith(`..${sep}`) ||
		isAbsolute(relativePath)
	) {
		throw new Error(`Documentation pairing path must stay inside the repository: ${input}`);
	}
	const normalizedPath = relativePath.split(sep).join('/');
	const sourcePath =
		normalizedPath.startsWith('docs/zh-CN/') || normalizedPath.endsWith(LOCALIZED_MARKDOWN_SUFFIX)
			? sourcePathForLocalizedPath(normalizedPath)
			: normalizedPath;
	assertSourcePath(sourcePath);
	return sourcePath;
}

export function contentSha256(content) {
	return createHash('sha256').update(normalizeNewlines(content)).digest('hex');
}

function isEscaped(text, index) {
	let precedingBackslashes = 0;
	for (let candidate = index - 1; candidate >= 0 && text[candidate] === '\\'; candidate -= 1) {
		precedingBackslashes += 1;
	}
	return precedingBackslashes % 2 === 1;
}

function tableColumnCount(row) {
	const lastContentIndex = row.trimEnd().length - 1;
	let delimiterCount = 0;
	for (let index = 0; index < row.length; index += 1) {
		if (row[index] === '|' && !isEscaped(row, index)) delimiterCount += 1;
	}
	return (
		delimiterCount -
		(lastContentIndex >= 0 && row[lastContentIndex] === '|' && !isEscaped(row, lastContentIndex)
			? 1
			: 0)
	);
}

export function documentationStructure(markdown) {
	const normalizedMarkdown = normalizeNewlines(markdown).replace(
		/^<!-- documentation-pairing: english-sha256=[a-f0-9]{64} zh-cn-sha256=[a-f0-9]{64} -->(?:\n)?$/mu,
		'',
	);
	const matches = (pattern) =>
		Array.from(normalizedMarkdown.matchAll(pattern), (match) => match[0]);

	return {
		blocks: normalizedMarkdown
			.trim()
			.split(/\n\s*\n/u)
			.map(() => 'block'),
		fences: matches(/^```[^\n]*$/gmu),
		headings: matches(/^(?:#{1,6})\s+/gmu),
		inlineCode: Array.from(normalizedMarkdown.matchAll(/(?<!`)`([^`]+?)`(?!`)/gsu), (match) =>
			(match[1] ?? '').replaceAll(/\s+/gu, ' '),
		).toSorted(),
		links: matches(/\]\([^)]+\)/gu).map(() => 'link'),
		orderedItems: matches(/^\d+\.\s+/gmu),
		tableColumns: matches(/^\|[^\n]*$/gmu).map(tableColumnCount),
		tableRows: matches(/^\|/gmu),
		unorderedItems: matches(/^-\s+/gmu),
	};
}

export function localizedPathForSource(sourcePath) {
	assertSourcePath(sourcePath);
	return sourcePath.startsWith('docs/')
		? `docs/zh-CN/${sourcePath.slice('docs/'.length)}`
		: `${sourcePath.slice(0, -'.md'.length)}${LOCALIZED_MARKDOWN_SUFFIX}`;
}

export function discoverDocumentationSources(repositoryRoot = REPOSITORY_ROOT) {
	return discoverRepositoryMarkdownPaths(repositoryRoot)
		.filter((path) => !path.startsWith('docs/zh-CN/') && !path.endsWith(LOCALIZED_MARKDOWN_SUFFIX))
		.toSorted();
}

function discoverLocalizedPairs(repositoryRoot) {
	const diagnostics = [];
	const pairs = [];
	for (const localizedPath of discoverRepositoryMarkdownPaths(repositoryRoot).filter(
		(path) => path.startsWith('docs/zh-CN/') || path.endsWith(LOCALIZED_MARKDOWN_SUFFIX),
	)) {
		let sourcePath;
		try {
			sourcePath = sourcePathForLocalizedPath(localizedPath);
		} catch (error) {
			diagnostics.push(`${localizedPath}: ${errorMessage(error)}`);
			continue;
		}
		const expectedLocalizedPath = localizedPathForSource(sourcePath);
		if (localizedPath !== expectedLocalizedPath) {
			diagnostics.push(
				`${localizedPath}: Simplified Chinese Markdown for ${sourcePath} must use ${expectedLocalizedPath}`,
			);
			continue;
		}
		pairs.push({ localizedPath, sourcePath });
	}
	return {
		diagnostics: diagnostics.toSorted(),
		pairs: pairs.toSorted((left, right) => comparePaths(left.sourcePath, right.sourcePath)),
	};
}

function readPairingReceipt(markdown) {
	const normalized = normalizeNewlines(markdown);
	const markers = [
		...normalized.matchAll(/^[ \t]*<!--\s*documentation-pairing\b[^\n]*(?:\n|$)/gmu),
	];
	if (markers.length === 0) return { content: normalized, receipt: undefined };
	if (markers.length !== 1) {
		throw new Error('documentation pairing receipt must occur exactly once');
	}
	const [marker] = markers;
	const receipt = RECEIPT_LINE.exec(marker[0].replace(/\n$/u, ''));
	if (receipt === null) {
		throw new Error(
			'malformed documentation pairing receipt; expected lowercase SHA-256 digests on one HTML comment line',
		);
	}
	if (marker.index + marker[0].length !== normalized.length) {
		throw new Error('documentation pairing receipt must be the final line');
	}
	return {
		content: normalized.slice(0, marker.index),
		receipt: { englishSha256: receipt[1], zhCnSha256: receipt[2] },
	};
}

function renderPairingReceipt(english, chinese) {
	return `<!-- documentation-pairing: english-sha256=${contentSha256(english)} zh-cn-sha256=${contentSha256(chinese)} -->\n`;
}

export function collectDocumentationPairingDiagnostics({ repositoryRoot = REPOSITORY_ROOT } = {}) {
	const diagnostics = [];
	const sourcePaths = discoverDocumentationSources(repositoryRoot);
	const sourcePathSet = new Set(sourcePaths);
	const { diagnostics: localizedDiagnostics, pairs: localizedPairs } =
		discoverLocalizedPairs(repositoryRoot);
	diagnostics.push(...localizedDiagnostics);
	const localizedSourcePathSet = new Set(localizedPairs.map(({ sourcePath }) => sourcePath));

	for (const { localizedPath, sourcePath } of localizedPairs) {
		if (!sourcePathSet.has(sourcePath)) {
			diagnostics.push(`${localizedPath} has no English Markdown source`);
		}
	}

	for (const sourcePath of sourcePaths) {
		const localizedPath = localizedPathForSource(sourcePath);
		if (!localizedSourcePathSet.has(sourcePath)) {
			diagnostics.push(`${sourcePath}: missing localized counterpart ${localizedPath}`);
			continue;
		}

		const english = readFileSync(join(repositoryRoot, sourcePath), 'utf8');
		const chinese = readFileSync(join(repositoryRoot, localizedPath), 'utf8');
		if (!isDeepStrictEqual(documentationStructure(english), documentationStructure(chinese))) {
			diagnostics.push(`${sourcePath}: English and Simplified Chinese structures differ`);
		}
		let reviewed;
		try {
			reviewed = readPairingReceipt(chinese);
		} catch (error) {
			diagnostics.push(`${localizedPath}: ${errorMessage(error)}`);
			continue;
		}
		const { content, receipt } = reviewed;
		if (receipt === undefined) {
			diagnostics.push(
				`${localizedPath}: missing documentation pairing receipt; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
			continue;
		}
		if (contentSha256(english) !== receipt.englishSha256) {
			diagnostics.push(
				`${sourcePath}: English content changed after pairing confirmation; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
		}
		if (contentSha256(content) !== receipt.zhCnSha256) {
			diagnostics.push(
				`${localizedPath}: Simplified Chinese content changed after pairing confirmation; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
		}
	}

	return diagnostics.toSorted();
}

export function confirmDocumentationPairs({ inputs, repositoryRoot = REPOSITORY_ROOT }) {
	if (!Array.isArray(inputs) || inputs.length === 0) {
		throw new Error('Confirm requires at least one explicit English or Simplified Chinese path');
	}
	const sourcePaths = [
		...new Set(inputs.map((input) => sourcePathFromInput(repositoryRoot, input))),
	].toSorted();
	const writes = new Map();
	const changes = [];

	for (const sourcePath of sourcePaths) {
		const localizedPath = localizedPathForSource(sourcePath);
		const englishFile = join(repositoryRoot, sourcePath);
		const chineseFile = join(repositoryRoot, localizedPath);
		const englishExists = repositoryPathEntryExists(repositoryRoot, sourcePath);
		const chineseExists = repositoryPathEntryExists(repositoryRoot, localizedPath);
		if (englishExists !== chineseExists) {
			const missingPath = englishExists ? localizedPath : sourcePath;
			throw new Error(`${sourcePath}: cannot confirm while ${missingPath} is missing`);
		}
		if (!englishExists) {
			changes.push({ sourcePath, status: 'removed' });
			continue;
		}

		// Write through in-repository aliases, so AGENTS keeps sharing CLAUDE's receipt.
		const target = realpathSync(chineseFile);
		const relativeTarget = relative(realpathSync(repositoryRoot), target);
		if (
			relativeTarget === '..' ||
			relativeTarget.startsWith(`..${sep}`) ||
			isAbsolute(relativeTarget)
		) {
			throw new Error(`${localizedPath}: receipt target must stay inside the repository`);
		}
		const english = readFileSync(englishFile, 'utf8');
		const { content } = readPairingReceipt(readFileSync(chineseFile, 'utf8'));
		// Keep a blank separator so Markdown formatters do not change the reviewed body.
		const body = content.replace(/\n*$/u, '');
		const chinese = body === '' ? '' : `${body}\n\n`;
		if (!isDeepStrictEqual(documentationStructure(english), documentationStructure(chinese))) {
			throw new Error(
				`${sourcePath}: cannot confirm because English and Simplified Chinese structures differ`,
			);
		}
		const confirmed = chinese + renderPairingReceipt(english, chinese);
		if (writes.has(target) && writes.get(target) !== confirmed) {
			throw new Error(
				`${sourcePath}: aliased pairs cannot share different documentation pairing receipts`,
			);
		}
		writes.set(target, confirmed);
		changes.push({ sourcePath, status: 'confirmed' });
	}

	// Validate every requested pair before changing any document.
	for (const [target, content] of writes) writeFileSync(target, content, 'utf8');
	return changes;
}

function usage() {
	return `Usage:
  pnpm lint:documentation
  pnpm docs:confirm-pairing -- <english-or-zh-CN-markdown-path> [...]
`;
}

export function stripArgumentSeparator(arguments_) {
	return arguments_[0] === '--' ? arguments_.slice(1) : arguments_;
}

function main() {
	const [command, ...arguments_] = process.argv.slice(2);
	const inputs = stripArgumentSeparator(arguments_);
	if (command === 'check') {
		if (inputs.length > 0) throw new Error(usage());
		const diagnostics = collectDocumentationPairingDiagnostics();
		if (diagnostics.length > 0) {
			throw new Error(`Documentation pairing check failed:\n${diagnostics.join('\n')}`);
		}
		process.stdout.write(
			`Verified ${discoverDocumentationSources().length} documentation pairing receipt(s)\n`,
		);
		return;
	}
	if (command === 'confirm') {
		const changes = confirmDocumentationPairs({ inputs });
		process.stdout.write(
			`Updated ${changes.length} documentation pairing receipt(s): ${changes
				.map(({ sourcePath, status }) => `${sourcePath} (${status})`)
				.join(', ')}\n`,
		);
		return;
	}
	throw new Error(command === undefined ? usage() : `Unknown command "${command}"\n${usage()}`);
}

const entryPath = process.argv[1];
if (entryPath !== undefined && import.meta.url === pathToFileURL(resolve(entryPath)).href) {
	try {
		main();
	} catch (error) {
		process.stderr.write(`${errorMessage(error)}\n`);
		process.exitCode = 1;
	}
}
