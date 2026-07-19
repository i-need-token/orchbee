import { basename, extname } from 'node:path';
import { isTopLevelFunction, startLoc } from '../utils.js';

const COMPONENT_NAME = /^[A-Z][A-Za-z0-9]*$/u;
const HOOK_NAME = /^use[A-Z0-9][A-Za-z0-9]*$/u;

function shouldCheck(filePath) {
	const normalized = filePath.split('\\').join('/');
	const fileName = basename(normalized);
	return (
		fileName.endsWith('.tsx') &&
		!fileName.endsWith('.test.tsx') &&
		!fileName.endsWith('.spec.tsx') &&
		!fileName.endsWith('.stories.tsx') &&
		normalized.includes('/src/') &&
		!normalized.includes('/__tests__/')
	);
}

function identifierName(identifier) {
	return identifier?.type === 'Identifier' ? identifier.name : undefined;
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

function runtimeDeclarationNames(declaration) {
	if (declaration?.type === 'VariableDeclaration') {
		return declaration.declarations.map((declarator) => identifierName(declarator.id));
	}
	if (
		declaration?.type === 'TSInterfaceDeclaration' ||
		declaration?.type === 'TSTypeAliasDeclaration'
	)
		return [];
	const name = identifierName(declaration?.id);
	return name === undefined ? [] : [name];
}

/** @type {import('eslint').Rule.RuleModule} */
export const oneFunctionPerTsx = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Limit authored production TSX files to one top-level function.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const topLevelFunctions = [];
		const runtimeExports = [];

		function report(node, detail) {
			context.report({
				loc: startLoc(node.id ?? node),
				message: `${detail} See docs/frontend/react.md#tsx-file-boundary.`,
			});
		}

		function checkFunction(node) {
			if (!isTopLevelFunction(node)) return;
			topLevelFunctions.push(node);
			if (topLevelFunctions.length > 1)
				report(
					node,
					'Move this top-level function to its own module; an authored production TSX file may contain only one top-level function.',
				);
		}

		return {
			ArrowFunctionExpression: checkFunction,
			ExportAllDeclaration(node) {
				runtimeExports.push({ name: undefined, node });
			},
			ExportDefaultDeclaration(node) {
				runtimeExports.push({ name: 'default', node });
			},
			ExportNamedDeclaration(node) {
				if (node.exportKind === 'type') return;
				for (const name of runtimeDeclarationNames(node.declaration)) {
					runtimeExports.push({ name, node });
				}
				for (const specifier of node.specifiers ?? []) {
					if (specifier.exportKind === 'type') continue;
					runtimeExports.push({ name: identifierName(specifier.exported), node: specifier });
				}
			},
			FunctionDeclaration: checkFunction,
			FunctionExpression: checkFunction,
			'Program:exit'(node) {
				const fileStem = basename(context.filename, extname(context.filename));
				if (topLevelFunctions.length === 0) {
					if (COMPONENT_NAME.test(fileStem))
						report(
							node,
							`Rename "${basename(context.filename)}" to kebab-case because this TSX module does not define a function component.`,
						);
					return;
				}
				if (topLevelFunctions.length > 1) return;

				const componentFunction = topLevelFunctions[0];
				const name = functionName(componentFunction);
				if (name !== undefined && HOOK_NAME.test(name)) return;
				if (runtimeExports.length !== 1) {
					report(
						runtimeExports[1]?.node ?? runtimeExports[0]?.node ?? node,
						'This function-component module must have exactly one named runtime export.',
					);
					return;
				}
				if (name === undefined || !COMPONENT_NAME.test(name) || runtimeExports[0]?.name !== name) {
					report(
						componentFunction,
						'The sole function must be the named PascalCase component export.',
					);
					return;
				}

				if (fileStem !== name) {
					report(
						componentFunction,
						`Rename "${basename(context.filename)}" to "${name}.tsx" so the component file matches its export.`,
					);
				}
			},
		};
	},
};
