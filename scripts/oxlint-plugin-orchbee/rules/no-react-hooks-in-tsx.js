import { basename } from 'node:path';
import { startLoc } from '../utils.js';

const AUTHORITY = 'See docs/frontend/react.md#component-hook-boundary.';

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

function identifierName(node) {
	return node?.type === 'Identifier' ? node.name : undefined;
}

function isHookName(name) {
	return typeof name === 'string' && /^use(?:[A-Z0-9]|$)/u.test(name);
}

/** @type {import('eslint').Rule.RuleModule} */
export const noReactHooksInTsx = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Keep direct React Hook primitives out of authored production TSX files.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const reactNamespaces = new Set();
		const reactHookImports = new Set();

		function report(node, hookName) {
			context.report({
				loc: startLoc(node),
				message: `Move the direct React Hook "${hookName}" into a responsibility-focused custom Hook; authored production TSX only renders prepared component inputs. ${AUTHORITY}`,
			});
		}

		return {
			ImportDeclaration(node) {
				if (node.source?.value !== 'react') return;
				for (const specifier of node.specifiers ?? []) {
					const localName = identifierName(specifier.local);
					if (localName === undefined) continue;
					if (
						specifier.type === 'ImportNamespaceSpecifier' ||
						specifier.type === 'ImportDefaultSpecifier'
					) {
						reactNamespaces.add(localName);
						continue;
					}
					const importedName = identifierName(specifier.imported);
					if (isHookName(importedName) && specifier.importKind !== 'type') {
						reactHookImports.add(localName);
					}
				}
			},
			CallExpression(node) {
				const directName = identifierName(node.callee);
				if (directName !== undefined && reactHookImports.has(directName)) {
					report(node.callee, directName);
					return;
				}

				if (node.callee?.type !== 'MemberExpression' || node.callee.computed) return;
				const namespaceName = identifierName(node.callee.object);
				const hookName = identifierName(node.callee.property);
				if (
					namespaceName !== undefined &&
					reactNamespaces.has(namespaceName) &&
					isHookName(hookName)
				) {
					report(node.callee, `${namespaceName}.${hookName}`);
				}
			},
		};
	},
};
