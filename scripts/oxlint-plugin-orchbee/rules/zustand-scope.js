import { basename } from 'node:path';
import { isFunctionNode, startLoc } from '../utils.js';

function shouldCheck(filePath) {
	const normalized = filePath.split('\\').join('/');
	const fileName = basename(normalized);
	return (
		(fileName.endsWith('.ts') || fileName.endsWith('.tsx')) &&
		!fileName.endsWith('.d.ts') &&
		!fileName.endsWith('.test.ts') &&
		!fileName.endsWith('.test.tsx') &&
		!fileName.endsWith('.spec.ts') &&
		!fileName.endsWith('.spec.tsx') &&
		normalized.includes('/src/') &&
		!normalized.includes('/__tests__/')
	);
}

function isContextStoreModule(filePath) {
	const normalized = filePath.split('\\').join('/');
	const fileName = basename(normalized);
	if (fileName === 'create-context-store.ts' || fileName === 'create-context-store.tsx') return true;
	return (
		normalized.includes('/contexts/') &&
		/^[a-z0-9]+(?:-[a-z0-9]+)*-store\.tsx?$/u.test(fileName)
	);
}

function importedName(specifier) {
	if (specifier.type !== 'ImportSpecifier') return undefined;
	const imported = specifier.imported;
	if (imported?.type === 'Identifier') return imported.name;
	if (imported?.type === 'StringLiteral' || imported?.type === 'Literal') return imported.value;
	return undefined;
}

function isTypeOnlyImport(declaration, specifier) {
	return declaration.importKind === 'type' || specifier.importKind === 'type';
}

function hasContainingFunction(node) {
	let ancestor = node.parent;
	while (ancestor) {
		if (isFunctionNode(ancestor)) return true;
		ancestor = ancestor.parent;
	}
	return false;
}

/** @type {import('eslint').Rule.RuleModule} */
export const zustandScope = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Keep authored Zustand stores scoped to mounted component providers.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const useStoreBindings = new Set();
		const vanillaCreateStoreBindings = new Set();
		function report(node, detail) {
			context.report({
				loc: startLoc(node),
				message: `${detail} Keep the Provider and selector Hook together in a context-scoped vanilla Store module. See docs/frontend/react.md#zustand-and-context.`,
			});
		}

		return {
			ImportDeclaration(node) {
				const source = node.source.value;
				if (source !== 'zustand' && source !== 'zustand/vanilla') return;

				for (const specifier of node.specifiers) {
					if (isTypeOnlyImport(node, specifier)) continue;
					const name = importedName(specifier);

					if (source === 'zustand') {
						if (specifier.type !== 'ImportSpecifier') {
							report(specifier, 'Default and namespace Zustand imports hide Store ownership.');
							continue;
						}
						if (name === 'create' || name === 'createStore') {
							report(
								specifier,
								'Bound or React-entry Store creation can produce a process-wide singleton.',
							);
						}
						if (name === 'useStore') {
							useStoreBindings.add(specifier.local.name);
							if (!isContextStoreModule(context.filename)) {
								report(
									specifier,
									'Direct useStore imports are limited to component contexts/*-store modules or a shared create-context-store factory.',
								);
							}
						}
						continue;
					}

					if (name === 'createStore') {
						vanillaCreateStoreBindings.add(specifier.local.name);
					}
				}
			},
			CallExpression(node) {
				if (node.callee.type !== 'Identifier') return;
				if (vanillaCreateStoreBindings.has(node.callee.name) && !hasContainingFunction(node)) {
					report(node.callee, 'A module-level Store instance is shared across feature mounts.');
				}
				if (useStoreBindings.has(node.callee.name) && node.arguments.length < 2) {
					report(node.callee, 'Every useStore subscription requires an explicit selector.');
				}
			},
		};
	},
};
