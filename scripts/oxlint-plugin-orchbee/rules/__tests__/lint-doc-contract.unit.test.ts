import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { describe, expect, it } from 'vitest';
import { documentationStructure } from '../../../documentation-pairing.mjs';
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

function englishDocumentationFiles(): readonly string[] {
	return collectFiles(DOCS_ROOT, '.md').filter(
		(file) => !file.startsWith(`${LOCALIZED_DOCS_ROOT}${sep}`),
	);
}

function documentAnchors(documentPath: string): ReadonlySet<string> {
	const markdown = readFileSync(documentPath, 'utf8');
	const headings = markdown.matchAll(/^#{1,6}\s+(.+)$/gm);
	return new Set(Array.from(headings, (match) => markdownAnchor(match[1] ?? '')));
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

	it('keeps localized Markdown links valid inside the Chinese mirror', () => {
		for (const documentPath of collectFiles(LOCALIZED_DOCS_ROOT, '.md')) {
			const markdown = readFileSync(documentPath, 'utf8');
			const targets = Array.from(
				markdown.matchAll(/\]\(([^)]+)\)/gu),
				(match) => match[1] ?? '',
			).filter((target) => !/^(?:https?:|mailto:)/u.test(target));

			for (const target of targets) {
				const [relativeTarget = '', encodedAnchor] = target.split('#');
				const targetPath = relativeTarget
					? resolve(dirname(documentPath), decodeURIComponent(relativeTarget))
					: documentPath;
				expect({
					link: `${relative(DOCS_ROOT, documentPath)} -> ${target}`,
					valid: existsSync(targetPath),
				}).toEqual({
					link: `${relative(DOCS_ROOT, documentPath)} -> ${target}`,
					valid: true,
				});
				if (encodedAnchor === undefined || !targetPath.endsWith('.md')) continue;
				expect({
					link: `${relative(DOCS_ROOT, documentPath)} -> ${target}`,
					valid: documentAnchors(targetPath).has(decodeURIComponent(encodedAnchor)),
				}).toEqual({
					link: `${relative(DOCS_ROOT, documentPath)} -> ${target}`,
					valid: true,
				});
			}
		}
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
