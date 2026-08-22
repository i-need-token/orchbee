import { createHash, randomUUID } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const REPOSITORY_ROOT = resolve(import.meta.dirname, '..');
const FORMAT_VERSION = 1;
const LOCALIZED_MARKDOWN_SUFFIX = '.zh-CN.md';
const SHA_256_PATTERN = /^[a-f0-9]{64}$/u;

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

function repositoryPath(repositoryRoot, absolutePath) {
	return relative(repositoryRoot, absolutePath).split(sep).join('/');
}

function displayPath(repositoryRoot, absolutePath) {
	const candidate = repositoryPath(repositoryRoot, absolutePath);
	return candidate === '..' || candidate.startsWith('../') ? absolutePath : candidate;
}

function pathEntryExists(path) {
	try {
		lstatSync(path);
		return true;
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
		.filter((path) => pathEntryExists(join(repositoryRoot, path)))
		.toSorted();
}

function assertRecord(value, label) {
	if (value === null || typeof value !== 'object' || Array.isArray(value)) {
		throw new Error(`${label} must be an object`);
	}
}

function assertExactKeys(value, expectedKeys, label) {
	const actualKeys = Object.keys(value).toSorted();
	if (!isDeepStrictEqual(actualKeys, expectedKeys.toSorted())) {
		throw new Error(`${label} must contain exactly: ${expectedKeys.join(', ')}`);
	}
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

function readManifest(manifestPath) {
	if (!existsSync(manifestPath)) {
		throw new Error('documentation pairing manifest does not exist');
	}
	const raw = readFileSync(manifestPath, 'utf8');
	return { manifest: parsePairingManifest(raw), raw };
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

function writeManifestAtomically(manifestPath, manifest) {
	const temporaryPath = `${manifestPath}.${process.pid}.${randomUUID()}.tmp`;
	try {
		writeFileSync(temporaryPath, renderPairingManifest(manifest), {
			encoding: 'utf8',
			flag: 'wx',
			mode: 0o644,
		});
		renameSync(temporaryPath, manifestPath);
	} finally {
		rmSync(temporaryPath, { force: true });
	}
}

export function contentSha256(content) {
	return createHash('sha256').update(normalizeNewlines(content)).digest('hex');
}

export function documentationStructure(markdown) {
	const normalizedMarkdown = normalizeNewlines(markdown);
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

export function parsePairingManifest(raw) {
	let value;
	try {
		value = JSON.parse(raw);
	} catch (error) {
		throw new Error(`documentation pairing manifest is not valid JSON: ${errorMessage(error)}`, {
			cause: error,
		});
	}
	assertRecord(value, 'Documentation pairing manifest');
	assertExactKeys(value, ['formatVersion', 'pairs'], 'Documentation pairing manifest');
	if (value.formatVersion !== FORMAT_VERSION) {
		throw new Error(`Documentation pairing formatVersion must be ${FORMAT_VERSION}`);
	}
	assertRecord(value.pairs, 'Documentation pairing pairs');

	const pairs = {};
	for (const [sourcePath, receipt] of Object.entries(value.pairs)) {
		assertSourcePath(sourcePath);
		assertRecord(receipt, `Documentation pairing receipt for ${sourcePath}`);
		assertExactKeys(
			receipt,
			['englishSha256', 'zhCnSha256'],
			`Documentation pairing receipt for ${sourcePath}`,
		);
		if (!SHA_256_PATTERN.test(receipt.englishSha256)) {
			throw new Error(`${sourcePath} englishSha256 must be a lowercase SHA-256 digest`);
		}
		if (!SHA_256_PATTERN.test(receipt.zhCnSha256)) {
			throw new Error(`${sourcePath} zhCnSha256 must be a lowercase SHA-256 digest`);
		}
		pairs[sourcePath] = {
			englishSha256: receipt.englishSha256,
			zhCnSha256: receipt.zhCnSha256,
		};
	}
	return { formatVersion: FORMAT_VERSION, pairs };
}

export function renderPairingManifest(manifest) {
	const pairs = Object.fromEntries(
		Object.entries(manifest.pairs)
			.toSorted(([left], [right]) => comparePaths(left, right))
			.map(([sourcePath, receipt]) => [
				sourcePath,
				{
					englishSha256: receipt.englishSha256,
					zhCnSha256: receipt.zhCnSha256,
				},
			]),
	);
	return `${JSON.stringify({ formatVersion: FORMAT_VERSION, pairs }, null, '\t')}\n`;
}

export function collectDocumentationPairingDiagnostics({
	repositoryRoot = REPOSITORY_ROOT,
	manifestPath,
} = {}) {
	const resolvedManifestPath =
		manifestPath ?? join(repositoryRoot, 'scripts', 'documentation-pairing.json');
	const manifestDisplayPath = displayPath(repositoryRoot, resolvedManifestPath);
	let manifest;
	let rawManifest;
	try {
		({ manifest, raw: rawManifest } = readManifest(resolvedManifestPath));
	} catch (error) {
		return [`${manifestDisplayPath}: ${errorMessage(error)}`];
	}

	const diagnostics = [];
	if (normalizeNewlines(rawManifest) !== renderPairingManifest(manifest)) {
		diagnostics.push(`${manifestDisplayPath}: documentation pairing manifest is not canonical`);
	}

	const sourcePaths = discoverDocumentationSources(repositoryRoot);
	const sourcePathSet = new Set(sourcePaths);
	const { diagnostics: localizedDiagnostics, pairs: localizedPairs } =
		discoverLocalizedPairs(repositoryRoot);
	diagnostics.push(...localizedDiagnostics);
	const localizedSourcePaths = localizedPairs.map(({ sourcePath }) => sourcePath);
	const localizedSourcePathSet = new Set(localizedSourcePaths);
	const receiptPaths = Object.keys(manifest.pairs).toSorted();

	for (const { localizedPath, sourcePath } of localizedPairs) {
		if (!sourcePathSet.has(sourcePath)) {
			diagnostics.push(`${localizedPath} has no English Markdown source`);
		}
	}
	for (const receiptPath of receiptPaths) {
		if (!sourcePathSet.has(receiptPath)) {
			diagnostics.push(
				`${manifestDisplayPath}: receipt has no English Markdown source: ${receiptPath}`,
			);
		}
	}

	for (const sourcePath of sourcePaths) {
		const localizedPath = localizedPathForSource(sourcePath);
		const receipt = manifest.pairs[sourcePath];
		if (!localizedSourcePathSet.has(sourcePath)) {
			diagnostics.push(`${sourcePath}: missing localized counterpart ${localizedPath}`);
		}
		if (receipt === undefined) {
			diagnostics.push(
				`${sourcePath}: missing documentation pairing receipt; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
		}
		if (!localizedSourcePathSet.has(sourcePath)) continue;

		const english = readFileSync(join(repositoryRoot, sourcePath), 'utf8');
		const chinese = readFileSync(join(repositoryRoot, localizedPath), 'utf8');
		if (!isDeepStrictEqual(documentationStructure(english), documentationStructure(chinese))) {
			diagnostics.push(`${sourcePath}: English and Simplified Chinese structures differ`);
		}
		if (receipt === undefined) continue;
		if (contentSha256(english) !== receipt.englishSha256) {
			diagnostics.push(
				`${sourcePath}: English content changed after pairing confirmation; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
		}
		if (contentSha256(chinese) !== receipt.zhCnSha256) {
			diagnostics.push(
				`${localizedPath}: Simplified Chinese content changed after pairing confirmation; review the pair, then run pnpm docs:confirm-pairing -- ${sourcePath}`,
			);
		}
	}

	return diagnostics.toSorted();
}

export function confirmDocumentationPairs({
	inputs,
	repositoryRoot = REPOSITORY_ROOT,
	manifestPath,
}) {
	if (!Array.isArray(inputs) || inputs.length === 0) {
		throw new Error('Confirm requires at least one explicit English or Simplified Chinese path');
	}
	const resolvedManifestPath =
		manifestPath ?? join(repositoryRoot, 'scripts', 'documentation-pairing.json');
	const { manifest } = readManifest(resolvedManifestPath);
	const sourcePaths = [
		...new Set(inputs.map((input) => sourcePathFromInput(repositoryRoot, input))),
	].toSorted();
	const pairs = { ...manifest.pairs };
	const changes = [];

	for (const sourcePath of sourcePaths) {
		const localizedPath = localizedPathForSource(sourcePath);
		const englishFile = join(repositoryRoot, sourcePath);
		const chineseFile = join(repositoryRoot, localizedPath);
		const englishExists = pathEntryExists(englishFile);
		const chineseExists = pathEntryExists(chineseFile);
		if (englishExists !== chineseExists) {
			const missingPath = englishExists ? localizedPath : sourcePath;
			throw new Error(`${sourcePath}: cannot confirm while ${missingPath} is missing`);
		}

		if (!englishExists) {
			if (!Object.hasOwn(pairs, sourcePath)) {
				throw new Error(`${sourcePath}: cannot confirm deletion without an existing receipt`);
			}
			delete pairs[sourcePath];
			changes.push({ sourcePath, status: 'removed' });
			continue;
		}

		const english = readFileSync(englishFile, 'utf8');
		const chinese = readFileSync(chineseFile, 'utf8');
		if (!isDeepStrictEqual(documentationStructure(english), documentationStructure(chinese))) {
			throw new Error(
				`${sourcePath}: cannot confirm because English and Simplified Chinese structures differ`,
			);
		}
		pairs[sourcePath] = {
			englishSha256: contentSha256(english),
			zhCnSha256: contentSha256(chinese),
		};
		changes.push({ sourcePath, status: 'confirmed' });
	}

	writeManifestAtomically(resolvedManifestPath, { formatVersion: FORMAT_VERSION, pairs });
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
