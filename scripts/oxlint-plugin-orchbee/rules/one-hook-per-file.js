import { basename } from 'node:path';
import { isTopLevelFunction, startLoc } from '../utils.js';

function shouldCheck(filePath) {
	const normalized = filePath.split('\\').join('/');
	const fileName = basename(normalized);
	return /^use-[a-z0-9]+(?:-[a-z0-9]+)*\.tsx?$/u.test(fileName) && normalized.includes('/src/');
}

function identifierName(identifier) {
	if (identifier?.type !== 'Identifier') return undefined;
	return identifier.name;
}

function functionName(node) {
	const declaredName = identifierName(node.id);
	if (declaredName !== undefined) return declaredName;

	let ancestor = node.parent;
	while (ancestor && ancestor.type !== 'Program') {
		if (ancestor.type === 'VariableDeclarator') return identifierName(ancestor.id);
		ancestor = ancestor.parent;
	}
	return undefined;
}

function declarationExportNames(declaration) {
	if (declaration?.type === 'VariableDeclaration') {
		return declaration.declarations.map((declarator) => identifierName(declarator.id));
	}
	const name = identifierName(declaration?.id);
	return name === undefined ? [] : [name];
}

/** @type {import('eslint').Rule.RuleModule} */
export const oneHookPerFile = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Require Hook modules to own one top-level function and one export.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const topLevelFunctions = [];
		const exportedSymbols = [];
		function recordFunction(node) {
			if (isTopLevelFunction(node)) topLevelFunctions.push(node);
		}

		function report(node, detail) {
			context.report({
				loc: startLoc(node),
				message: `${detail} Keep exactly one top-level Hook and one exported symbol in an authored Hook module. See docs/frontend/react.md#hook-file-boundary.`,
			});
		}

		return {
			ArrowFunctionExpression: recordFunction,
			FunctionDeclaration: recordFunction,
			FunctionExpression: recordFunction,
			ExportAllDeclaration(node) {
				exportedSymbols.push({ name: undefined, node });
			},
			ExportDefaultDeclaration(node) {
				exportedSymbols.push({ name: 'default', node });
			},
			ExportNamedDeclaration(node) {
				for (const name of declarationExportNames(node.declaration)) {
					exportedSymbols.push({ name, node });
				}
				for (const specifier of node.specifiers ?? []) {
					exportedSymbols.push({ name: identifierName(specifier.exported), node: specifier });
				}
			},
			'Program:exit'(node) {
				if (topLevelFunctions.length !== 1) {
					const locationNode = topLevelFunctions[1] ?? topLevelFunctions[0] ?? node;
					report(locationNode.id ?? locationNode, 'This module has the wrong number of functions.');
					return;
				}
				if (exportedSymbols.length !== 1) {
					const locationNode = exportedSymbols[1]?.node ?? exportedSymbols[0]?.node ?? node;
					report(locationNode, 'This module has the wrong number of exports.');
					return;
				}

				const hookName = functionName(topLevelFunctions[0]);
				const exportName = exportedSymbols[0]?.name;
				if (hookName === undefined || !/^use[A-Z0-9]/u.test(hookName) || exportName !== hookName) {
					report(
						topLevelFunctions[0].id ?? topLevelFunctions[0],
						'The sole function must be the named Hook export.',
					);
				}
			},
		};
	},
};
