import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	discoverDocumentationSources,
	documentationStructure,
	localizedPathForSource,
} from '../../../documentation-pairing.mjs';
import orchbeePlugin from '../../index';

interface OxlintConfig {
	readonly rules: Readonly<Record<string, unknown>>;
	readonly overrides?: readonly {
		readonly rules?: Readonly<Record<string, unknown>>;
	}[];
}

const REPOSITORY_ROOT = resolve(import.meta.dirname, '../../../..');
const DOCS_ROOT = join(REPOSITORY_ROOT, 'docs');
const LOCALIZED_DOCS_ROOT = join(DOCS_ROOT, 'zh-CN');
const RULES_ROOT = resolve(import.meta.dirname, '..');
const STANDARD_DOMAIN_DIRECTORIES = new Set(['cli', 'frontend', 'tui']);

function collectFiles(directory: string, extension: string): readonly string[] {
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) return collectFiles(path, extension);
		return entry.name.endsWith(extension) ? [path] : [];
	});
}

function markdownAnchor(heading: string): string {
	return heading
		.trim()
		.toLowerCase()
		.replace(/[^\p{Letter}\p{Number}\s_-]/gu, '')
		.replace(/\s+/g, '-');
}

function markdownOutsideFences(markdown: string): string {
	let fence: string | undefined;
	return markdown
		.split(/\r?\n/u)
		.map((line) => {
			const delimiter = /^ {0,3}(`{3,}|~{3,})/u.exec(line)?.[1];
			if (fence !== undefined) {
				if (
					delimiter !== undefined &&
					delimiter[0] === fence[0] &&
					delimiter.length >= fence.length &&
					line.trim() === delimiter
				) {
					fence = undefined;
				}
				return '';
			}
			if (delimiter !== undefined) {
				fence = delimiter;
				return '';
			}
			return line;
		})
		.join('\n');
}

function localMarkdownTargets(markdown: string): readonly string[] {
	const prose = markdownOutsideFences(markdown).replace(
		/(?<!`)(`+)(?!`)[\s\S]*?(?<!`)\1(?!`)/gu,
		'',
	);
	return Array.from(
		prose.matchAll(/\]\(\s*(?:<([^>]+)>|([^\s)]+))(?:\s+(?:"[^"]*"|'[^']*'))?\s*\)/gu),
		(match) => match[1] ?? match[2] ?? '',
	).filter((target) => !/^(?:[a-z][a-z\d+.-]*:|\/\/)/iu.test(target));
}

function englishDocumentationFiles(): readonly string[] {
	return collectFiles(DOCS_ROOT, '.md').filter(
		(file) => !file.startsWith(`${LOCALIZED_DOCS_ROOT}${sep}`),
	);
}

function documentAnchors(documentPath: string): ReadonlySet<string> {
	const markdown = markdownOutsideFences(readFileSync(documentPath, 'utf8'));
	const anchors = new Set<string>();
	for (const match of markdown.matchAll(/^ {0,3}#{1,6}\s+(.+?)(?:\s+#+)?\s*$/gm)) {
		const base = markdownAnchor(match[1] ?? '');
		let anchor = base;
		for (let duplicate = 1; anchors.has(anchor); duplicate += 1) anchor = `${base}-${duplicate}`;
		anchors.add(anchor);
	}
	for (const match of markdown.matchAll(/<a\s+(?:id|name)=["']([^"']+)["']/gu)) {
		if (match[1] !== undefined) anchors.add(match[1]);
	}
	return anchors;
}

function lintConfig(): OxlintConfig {
	const configPath = join(REPOSITORY_ROOT, '.oxlintrc.json');
	return JSON.parse(readFileSync(configPath, 'utf8')) as OxlintConfig;
}

function configuredRules(): Readonly<Record<string, unknown>> {
	return lintConfig().rules;
}

function explicitlyEnabledRules(): readonly string[] {
	const config = lintConfig();
	const ruleSets = [
		config.rules,
		...(config.overrides ?? []).map((override) => override.rules ?? {}),
	];
	return [
		...new Set(
			ruleSets.flatMap((rules) =>
				Object.entries(rules)
					.filter(([, setting]) => setting !== 'off')
					.map(([rule]) => rule),
			),
		),
	];
}

function documentedEnforcedRules(): readonly string[] {
	return [
		...new Set(
			englishDocumentationFiles().flatMap((file) => {
				const documentation = readFileSync(file, 'utf8');
				return Array.from(
					documentation.matchAll(/^- `([a-z][a-z0-9-]*\/[a-z0-9-]+)`:/gm),
					(match) => match[1] ?? '',
				);
			}),
		),
	];
}

function diagnosticAuthorities(): Readonly<Record<string, string>> {
	return Object.fromEntries(
		Object.keys(orchbeePlugin.rules).map((rule) => {
			const ruleFile = join(RULES_ROOT, `${rule}.js`);
			const source = readFileSync(ruleFile, 'utf8');
			const links = Array.from(
				source.matchAll(/See (docs\/[A-Za-z0-9_./-]+\.md)#([a-z0-9-]+)/g),
				(match) => `${match[1] ?? ''}#${match[2] ?? ''}`,
			);
			const authorities = [...new Set(links)];
			const [authority] = authorities;

			if (authority === undefined || authorities.length !== 1) {
				throw new Error(`${ruleFile} must reference exactly one authoritative docs target`);
			}

			return [`orchbee/${rule}`, authority];
		}),
	);
}

describe('lint-doc contract', () => {
	it('admits only indexed root standards and reviewed engineering-domain directories', () => {
		const claudeGuide = readFileSync(join(REPOSITORY_ROOT, 'CLAUDE.md'), 'utf8');
		const invalidPaths = [];

		for (const file of englishDocumentationFiles()) {
			const relativePath = relative(DOCS_ROOT, file);
			const segments = relativePath.split(sep);
			if (segments.length === 1) {
				if (!/^[A-Z][A-Z0-9-]*\.md$/u.test(relativePath)) {
					invalidPaths.push(relativePath);
					continue;
				}
				if (!claudeGuide.includes(`\`docs/${relativePath}\``)) invalidPaths.push(relativePath);
				continue;
			}

			const [domain, ...nestedPath] = segments;
			const fileName = nestedPath.at(-1) ?? '';
			const directories = nestedPath.slice(0, -1);
			if (
				domain === undefined ||
				!STANDARD_DOMAIN_DIRECTORIES.has(domain) ||
				!directories.every((directory) => /^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(directory)) ||
				!/^[a-z0-9]+(?:-[a-z0-9]+)*\.md$/u.test(fileName)
			) {
				invalidPaths.push(relativePath);
			}
		}

		expect(invalidPaths).toEqual([]);
	});

	it('keeps future workspace topology out of current standards', () => {
		const existingWorkspaces = new Set(
			['apps', 'packages'].flatMap((workspaceRoot) =>
				readdirSync(join(REPOSITORY_ROOT, workspaceRoot), { withFileTypes: true })
					.filter((entry) => entry.isDirectory())
					.map((entry) => `${workspaceRoot}/${entry.name}`),
			),
		);
		const missingWorkspaceReferences = [];

		for (const file of englishDocumentationFiles()) {
			const relativePath = relative(DOCS_ROOT, file);
			const lines = readFileSync(file, 'utf8').split(/\r?\n/u);
			for (const [lineIndex, line] of lines.entries()) {
				if (line.includes('❌') || line.includes('✅')) continue;
				for (const match of line.matchAll(/\b(?:apps|packages)\/[a-z0-9][a-z0-9-]*/gu)) {
					if (!existingWorkspaces.has(match[0])) {
						missingWorkspaceReferences.push(`${relativePath}:${lineIndex + 1}: ${match[0]}`);
					}
				}
			}
		}

		expect(missingWorkspaceReferences).toEqual([]);
	});

	it('keeps the Simplified Chinese documentation mirror complete and structurally synchronized', () => {
		const englishFiles = englishDocumentationFiles();
		const englishPaths = englishFiles.map((file) => relative(DOCS_ROOT, file)).toSorted();
		const localizedFiles = collectFiles(LOCALIZED_DOCS_ROOT, '.md');
		const localizedPaths = localizedFiles
			.map((file) => relative(LOCALIZED_DOCS_ROOT, file))
			.toSorted();

		expect(localizedPaths).toEqual(englishPaths);
		for (const englishFile of englishFiles) {
			const relativePath = relative(DOCS_ROOT, englishFile);
			const localizedFile = join(LOCALIZED_DOCS_ROOT, relativePath);
			const englishMarkdown = readFileSync(englishFile, 'utf8');
			const localizedMarkdown = readFileSync(localizedFile, 'utf8');

			expect({
				file: relativePath,
				structure: documentationStructure(localizedMarkdown),
			}).toEqual({
				file: relativePath,
				structure: documentationStructure(englishMarkdown),
			});
		}
	});

	it('checks real local links while excluding code examples and external destinations', () => {
		expect(
			localMarkdownTargets(
				[
					'`[inline example](missing.md)` and ``[code with ` a tick](missing.md)``',
					'````md',
					'[fenced example](missing.md)',
					'```',
					'[still fenced](missing.md)',
					'````',
					'~~~',
					'[tilde example](missing.md)',
					'~~~',
					'[external](https://example.com) [email](mailto:example@example.com)',
					'[protocol-relative](//example.com) [custom](app://example)',
					'[guide](../README.md#ownership "Guide") ![image](<assets/a b.png>)',
				].join('\n'),
			),
		).toEqual(['../README.md#ownership', 'assets/a b.png']);
	});

	it('keeps local links valid across repository Markdown pairs', () => {
		const documents = discoverDocumentationSources(REPOSITORY_ROOT).flatMap((sourcePath) => [
			sourcePath,
			localizedPathForSource(sourcePath),
		]);
		const anchorsByPath = new Map<string, ReadonlySet<string>>();
		const invalidLinks = [];
		for (const document of documents) {
			const documentPath = join(REPOSITORY_ROOT, document);
			for (const target of localMarkdownTargets(readFileSync(documentPath, 'utf8'))) {
				const [relativeTarget = '', encodedAnchor] = target.split('#');
				const pathTarget = relativeTarget.split('?')[0] ?? '';
				const targetPath = pathTarget
					? resolve(dirname(documentPath), decodeURIComponent(pathTarget))
					: documentPath;
				if (!existsSync(targetPath)) {
					invalidLinks.push(`${document} -> ${target}: missing path`);
					continue;
				}
				if (!encodedAnchor || !targetPath.endsWith('.md')) continue;
				const anchors = anchorsByPath.get(targetPath) ?? documentAnchors(targetPath);
				anchorsByPath.set(targetPath, anchors);
				if (!anchors.has(decodeURIComponent(encodedAnchor))) {
					invalidLinks.push(`${document} -> ${target}: missing heading`);
				}
			}
		}
		expect(invalidLinks).toEqual([]);
	});

	it('keeps every custom diagnostic docs anchor valid', () => {
		const authorities = Object.entries(diagnosticAuthorities());

		expect(authorities).not.toEqual([]);
		for (const [rule, authority] of authorities) {
			const [document = '', anchor = ''] = authority.split('#');
			const documentPath = join(REPOSITORY_ROOT, document);
			expect({
				link: `${rule}: ${document}`,
				valid: existsSync(documentPath),
			}).toEqual({ link: `${rule}: ${document}`, valid: true });
			expect({
				link: `${rule}: ${authority}`,
				valid: documentAnchors(documentPath).has(anchor),
			}).toEqual({ link: `${rule}: ${authority}`, valid: true });
		}
	});

	it('documents every explicitly enabled rule', () => {
		const documentation = englishDocumentationFiles()
			.map((file) => readFileSync(file, 'utf8'))
			.join('\n');
		const undocumentedRules = explicitlyEnabledRules().filter(
			(rule) => !documentation.includes(`\`${rule}\``),
		);

		expect(undocumentedRules).toEqual([]);
	});

	it('enables every rule documented as enforced', () => {
		const enabledRules = new Set(explicitlyEnabledRules());
		const disabledDocumentedRules = documentedEnforcedRules().filter(
			(rule) => !enabledRules.has(rule),
		);

		expect(disabledDocumentedRules).toEqual([]);
	});

	it('keeps custom plugin exports, config, and ownership docs synchronized', () => {
		const configuredCustomRules = Object.entries(configuredRules())
			.filter(([rule, setting]) => rule.startsWith('orchbee/') && setting !== 'off')
			.map(([rule]) => rule)
			.toSorted();
		const exportedCustomRules = Object.keys(orchbeePlugin.rules)
			.map((rule) => `orchbee/${rule}`)
			.toSorted();
		const lintingDoc = readFileSync(join(DOCS_ROOT, 'LINTING.md'), 'utf8');
		const documentedCustomRules = Array.from(
			lintingDoc.matchAll(/^\|\s*`(orchbee\/[^`]+)`\s*\|/gm),
			(match) => match[1] ?? '',
		).toSorted();
		const documentedAuthorities = Object.fromEntries(
			Array.from(
				lintingDoc.matchAll(
					/^\|\s*`(orchbee\/[^`]+)`\s*\|\s*`(docs\/[A-Za-z0-9_./-]+\.md#[a-z0-9-]+)`\s*\|/gm,
				),
				(match) => [match[1] ?? '', match[2] ?? ''],
			),
		);

		expect(configuredCustomRules).toEqual(exportedCustomRules);
		expect(configuredCustomRules).toEqual(documentedCustomRules);
		expect(diagnosticAuthorities()).toEqual(documentedAuthorities);
	});
});
