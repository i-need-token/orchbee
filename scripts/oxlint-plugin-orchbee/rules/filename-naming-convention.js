import { basename, dirname } from 'node:path';
import { startLoc } from '../utils.js';

const KEBAB_CASE = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;
const EXEMPT_DIRECTORIES = new Set(['src', '__tests__']);
const EXEMPT_FILES = new Set(['index']);
const UNIT_TEST_SUFFIXES = ['.unit.test.ts', '.unit.test.tsx'];

function normalize(value) {
	return value.split('\\').join('/');
}

function sourceRelativePath(filePath) {
	const normalized = normalize(filePath);
	const sourceIndex = normalized.indexOf('/src/');
	return sourceIndex === -1 ? null : normalized.slice(sourceIndex + 5);
}

function report(context, node, message) {
	context.report({ loc: startLoc(node), message });
}

function canonicalLocale(directory) {
	try {
		return new Intl.Locale(directory).toString();
	} catch {
		return null;
	}
}

function checkDirectories(context, node, relativePath) {
	const directories = dirname(relativePath).split('/');
	for (const [index, directory] of directories.entries()) {
		if (directory === '.' || EXEMPT_DIRECTORIES.has(directory)) continue;
		if (directories[index - 1] === 'locales') {
			const canonical = canonicalLocale(directory);
			if (canonical === directory) continue;
			const expected = canonical === null ? 'a canonical BCP 47 identifier' : `"${canonical}"`;
			report(
				context,
				node,
				`Rename locale directory "${directory}" to ${expected}. See docs/TESTING.md#source-and-test-paths.`,
			);
			continue;
		}
		if (!KEBAB_CASE.test(directory)) {
			report(
				context,
				node,
				`Rename directory "${directory}" to kebab-case. See docs/TESTING.md#source-and-test-paths.`,
			);
		}
	}
}

function checkTestFile(context, node, relativePath, fileName) {
	const isTest = /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(fileName);
	if (!isTest) return false;

	const unitTestSuffix = UNIT_TEST_SUFFIXES.find((suffix) => fileName.endsWith(suffix));
	if (!unitTestSuffix) {
		const expectedSuffix = fileName.endsWith('.tsx') ? '.unit.test.tsx' : '.unit.test.ts';
		report(
			context,
			node,
			`Rename unit test "${fileName}" to "<source-name>${expectedSuffix}". See docs/TESTING.md#source-and-test-paths.`,
		);
		return true;
	}

	if (basename(dirname(relativePath)) !== '__tests__') {
		report(
			context,
			node,
			`Move unit test "${fileName}" into the source file's sibling "__tests__" directory. See docs/TESTING.md#source-and-test-paths.`,
		);
	}

	const sourceStem = fileName.slice(0, -unitTestSuffix.length);
	if (!KEBAB_CASE.test(sourceStem)) {
		report(
			context,
			node,
			`Rename unit test "${fileName}" to preserve its source file's kebab-case name. See docs/TESTING.md#source-and-test-paths.`,
		);
	}
	return true;
}

function checkSourceFile(context, node, relativePath) {
	const fileName = basename(relativePath);
	if (fileName.endsWith('.d.ts') || checkTestFile(context, node, relativePath, fileName)) return;
	if (!/\.[cm]?[jt]sx?$/.test(fileName)) return;

	const extensionIndex = fileName.indexOf('.');
	const stem = extensionIndex === -1 ? fileName : fileName.slice(0, extensionIndex);
	if (EXEMPT_FILES.has(stem) || KEBAB_CASE.test(stem)) return;
	report(
		context,
		node,
		`Rename source file "${fileName}" to kebab-case. See docs/TESTING.md#source-and-test-paths.`,
	);
}

/** @type {import('eslint').Rule.RuleModule} */
export const filenameNamingConvention = {
	meta: {
		type: 'suggestion',
		docs: { description: 'Enforce source and unit-test path naming conventions.' },
		schema: [],
	},
	create(context) {
		const relativePath = sourceRelativePath(context.filename);
		if (!relativePath) return {};

		return {
			Program(node) {
				checkDirectories(context, node, relativePath);
				checkSourceFile(context, node, relativePath);
			},
		};
	},
};
