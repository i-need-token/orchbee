import { basename, dirname, extname } from 'node:path';
import { startLoc } from '../utils.js';

const KEBAB_CASE = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;
const CAMEL_CASE = /^[a-z][A-Za-z0-9]*$/;
const PASCAL_CASE = /^[A-Z][A-Za-z0-9]*$/;
const HOOK_CASE = /^use[A-Z0-9][A-Za-z0-9]*$/;
const EXEMPT_DIRECTORIES = new Set(['src', '__tests__']);
const EXEMPT_FILES = new Set(['index']);
const UNIT_TEST_SUFFIXES = ['.unit.test.ts', '.unit.test.tsx'];
const SYMBOL_MATCH_EXEMPT_PATHS = new Set(['packages/ui/src/hooks/use-mobile.ts']);

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

function identifierName(identifier) {
	if (identifier?.type === 'Identifier') return identifier.name;
	if (identifier?.type === 'Literal' && typeof identifier.value === 'string')
		return identifier.value;
	return undefined;
}

function declarationSymbols(declaration) {
	if (declaration === null || declaration === undefined) return [];
	if (declaration?.declare === true) return [];
	if (declaration?.type === 'FunctionDeclaration') {
		const name = identifierName(declaration.id);
		return name === undefined ? [] : [{ name, kind: 'function' }];
	}
	if (declaration?.type === 'ClassDeclaration') {
		const name = identifierName(declaration.id);
		return name === undefined ? [] : [{ name, kind: 'class' }];
	}
	if (declaration?.type === 'VariableDeclaration') {
		return declaration.declarations.flatMap((declarator) => {
			const name = identifierName(declarator.id);
			if (name === undefined) return [];
			if (
				declarator.init?.type === 'ArrowFunctionExpression' ||
				declarator.init?.type === 'FunctionExpression'
			)
				return [{ name, kind: 'function' }];
			if (declarator.init?.type === 'ClassExpression') return [{ name, kind: 'class' }];
			return [{ name, kind: 'other' }];
		});
	}
	if (
		declaration?.type === 'TSInterfaceDeclaration' ||
		declaration?.type === 'TSTypeAliasDeclaration'
	)
		return [];

	const name = identifierName(declaration?.id);
	return name === undefined ? [{ name: undefined, kind: 'other' }] : [{ name, kind: 'other' }];
}

function localSymbols(program) {
	const symbols = new Map();
	for (const statement of program.body ?? []) {
		const declaration =
			statement.type === 'ExportNamedDeclaration' ? statement.declaration : statement;
		for (const symbol of declarationSymbols(declaration)) {
			if (symbol.name !== undefined) symbols.set(symbol.name, symbol.kind);
		}
	}
	return symbols;
}

function runtimeExports(program) {
	const definitions = localSymbols(program);
	const exports = [];
	for (const statement of program.body ?? []) {
		if (statement.type === 'ExportDefaultDeclaration') {
			exports.push({ name: 'default', kind: 'other' });
			continue;
		}
		if (statement.type === 'ExportAllDeclaration') {
			if (statement.exportKind !== 'type') exports.push({ name: undefined, kind: 'other' });
			continue;
		}
		if (statement.type !== 'ExportNamedDeclaration' || statement.exportKind === 'type') continue;

		exports.push(...declarationSymbols(statement.declaration));
		for (const specifier of statement.specifiers ?? []) {
			if (specifier.exportKind === 'type') continue;
			const name = identifierName(specifier.exported);
			const localName = identifierName(specifier.local);
			const kind =
				statement.source === undefined && localName !== undefined
					? (definitions.get(localName) ?? 'other')
					: 'other';
			exports.push({ name, kind });
		}
	}
	return exports;
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
	const preservesSourceConvention =
		KEBAB_CASE.test(sourceStem) || CAMEL_CASE.test(sourceStem) || PASCAL_CASE.test(sourceStem);
	if (!preservesSourceConvention) {
		report(
			context,
			node,
			`Rename unit test "${fileName}" to preserve its source file's kebab-case module, camelCase function, or PascalCase component or class name. See docs/TESTING.md#source-and-test-paths.`,
		);
	}
	return true;
}

function checkSourceFile(context, node, relativePath, program) {
	const fileName = basename(relativePath);
	if (fileName.endsWith('.d.ts') || checkTestFile(context, node, relativePath, fileName)) return;
	if (!/\.[cm]?[jt]sx?$/.test(fileName)) return;

	const extensionIndex = fileName.indexOf('.');
	const stem = extensionIndex === -1 ? fileName : fileName.slice(0, extensionIndex);
	if (EXEMPT_FILES.has(stem)) return;

	const isComponentName = fileName.endsWith('.tsx') && PASCAL_CASE.test(stem);
	if (fileName.endsWith('.tsx')) {
		if (KEBAB_CASE.test(stem) || HOOK_CASE.test(stem) || isComponentName) return;
		report(
			context,
			node,
			`Rename source file "${fileName}" to kebab-case for an entry module, PascalCase for a function component, or camelCase for a Hook. See docs/TESTING.md#source-and-test-paths.`,
		);
		return;
	}

	const normalizedFilename = normalize(context.filename);
	const isGeneratedFile = fileName.split('.').includes('generated');
	const isSymbolMatchExempt =
		isGeneratedFile ||
		[...SYMBOL_MATCH_EXEMPT_PATHS].some((path) => normalizedFilename.endsWith(`/${path}`));
	const exports = isSymbolMatchExempt ? [] : runtimeExports(program);
	const primaryExport = exports.length === 1 ? exports[0] : undefined;
	const hasFunctionIdentity =
		primaryExport?.kind === 'function' &&
		primaryExport.name !== undefined &&
		CAMEL_CASE.test(primaryExport.name);
	const hasClassIdentity =
		primaryExport?.kind === 'class' &&
		primaryExport.name !== undefined &&
		PASCAL_CASE.test(primaryExport.name);

	if (hasFunctionIdentity || hasClassIdentity) {
		if (stem === primaryExport.name) return;
		report(
			context,
			node,
			`Rename source file "${fileName}" to "${primaryExport.name}${extname(fileName)}" so it matches its sole named runtime ${primaryExport.kind} export. See docs/TESTING.md#source-and-test-paths.`,
		);
		return;
	}

	if (KEBAB_CASE.test(stem)) return;
	report(
		context,
		node,
		`Rename source file "${fileName}" to kebab-case because it does not have one named runtime function or class export. See docs/TESTING.md#source-and-test-paths.`,
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
			},
			'Program:exit'(node) {
				checkSourceFile(context, node, relativePath, node);
			},
		};
	},
};
