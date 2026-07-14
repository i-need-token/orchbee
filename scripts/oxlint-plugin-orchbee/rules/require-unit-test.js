import { existsSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { startLoc } from '../utils.js';

function shouldSkip(filePath) {
	const normalized = filePath.split('\\').join('/');
	const fileName = basename(normalized);
	const isTypeScriptSource = fileName.endsWith('.ts') || fileName.endsWith('.tsx');
	return (
		!isTypeScriptSource ||
		fileName.endsWith('.d.ts') ||
		fileName.endsWith('.test.ts') ||
		fileName.endsWith('.test.tsx') ||
		fileName.endsWith('.spec.ts') ||
		fileName.endsWith('.spec.tsx') ||
		fileName === 'index.ts' ||
		fileName === 'index.tsx' ||
		!normalized.includes('/src/') ||
		normalized.includes('/__tests__/')
	);
}

function isTopLevelVariable(node) {
	const declaration = node.parent;
	if (declaration?.type !== 'VariableDeclaration') return false;
	const parent = declaration.parent;
	return (
		parent?.type === 'Program' ||
		(parent?.type === 'ExportNamedDeclaration' && parent.parent?.type === 'Program') ||
		(parent?.type === 'ExportDefaultDeclaration' && parent.parent?.type === 'Program')
	);
}

function isTestableInitializer(node) {
	return (
		node.init?.type === 'ArrowFunctionExpression' ||
		node.init?.type === 'FunctionExpression' ||
		node.init?.type === 'ClassExpression'
	);
}

/** @type {import('eslint').Rule.RuleModule} */
export const requireUnitTest = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Require function and class source files to have sibling unit tests.',
		},
		schema: [],
	},
	create(context) {
		const filePath = context.filename;
		if (shouldSkip(filePath)) return {};

		const extension = filePath.endsWith('.tsx') ? '.tsx' : '.ts';
		const stem = basename(filePath, extension);
		const testFile = `${stem}.unit.test${extension}`;
		const testPath = join(dirname(filePath), '__tests__', testFile);
		if (existsSync(testPath)) return {};

		let reported = false;
		function report(node) {
			if (reported) return;
			reported = true;
			context.report({
				loc: startLoc(node.id ?? node),
				message: `Add "__tests__/${testFile}" next to this source file; functions and classes require a sibling unit test. See docs/TESTING.md#required-unit-tests.`,
			});
		}

		return {
			FunctionDeclaration: report,
			ClassDeclaration: report,
			VariableDeclarator(node) {
				if (isTopLevelVariable(node) && isTestableInitializer(node)) report(node);
			},
		};
	},
};
