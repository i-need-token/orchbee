import { startLoc } from '../utils.js';

const AUTHORITY = 'See docs/frontend/react.md#callback-identity.';

function shouldCheck(filePath) {
	const normalized = filePath.split('\\').join('/');
	return (
		/\.(?:[cm]?[jt]s|[jt]sx)$/u.test(normalized) &&
		/(?:^|\/)(?:apps|packages)\/[^/]+\/src\//u.test(normalized)
	);
}

function identifierName(node) {
	return node?.type === 'Identifier' ? node.name : undefined;
}

function staticName(node) {
	if (node?.type === 'Identifier') return node.name;
	if (node?.type === 'Literal' || node?.type === 'StringLiteral') return node.value;
	return undefined;
}

function memberName(node) {
	if (node?.type !== 'MemberExpression') return undefined;
	return node.computed ? staticName(node.property) : identifierName(node.property);
}

function importedName(specifier) {
	if (specifier?.type !== 'ImportSpecifier') return undefined;
	return staticName(specifier.imported);
}

function exportedLocalName(specifier) {
	if (specifier?.type !== 'ExportSpecifier') return undefined;
	return staticName(specifier.local) ?? staticName(specifier.imported);
}

function objectPatternUseCallbackProperties(pattern) {
	if (pattern?.type !== 'ObjectPattern') return [];
	return (pattern.properties ?? []).filter(
		(property) => property.type === 'Property' && staticName(property.key) === 'useCallback',
	);
}

/** @type {import('eslint').Rule.RuleModule} */
export const noReactUseCallback = {
	meta: {
		type: 'problem',
		docs: {
			description: 'Disallow React useCallback throughout repository source.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const reactNamespaces = new Set();

		function report(node) {
			context.report({
				loc: startLoc(node),
				message: `React useCallback is prohibited. Use useMemoizedFn from "@orchbee/foundation/react" for a stable callback that reads the latest render values, and represent semantic changes as explicit data. ${AUTHORITY}`,
			});
		}

		function trackReactAlias(declaration, source) {
			if (source?.type !== 'Identifier' || !reactNamespaces.has(source.name)) return;
			if (declaration?.type === 'Identifier') {
				reactNamespaces.add(declaration.name);
				return;
			}
			for (const property of objectPatternUseCallbackProperties(declaration)) report(property);
		}

		return {
			ImportDeclaration(node) {
				if (node.source?.value !== 'react') return;
				for (const specifier of node.specifiers ?? []) {
					if (importedName(specifier) === 'useCallback') {
						report(specifier);
						continue;
					}
					if (
						specifier.type === 'ImportNamespaceSpecifier' ||
						specifier.type === 'ImportDefaultSpecifier'
					) {
						const localName = identifierName(specifier.local);
						if (localName !== undefined) reactNamespaces.add(localName);
					}
				}
			},
			ExportNamedDeclaration(node) {
				if (node.source?.value !== 'react') return;
				for (const specifier of node.specifiers ?? []) {
					if (exportedLocalName(specifier) === 'useCallback') report(specifier);
				}
			},
			ExportAllDeclaration(node) {
				if (node.source?.value === 'react') report(node);
			},
			MemberExpression(node) {
				if (
					node.object?.type === 'Identifier' &&
					reactNamespaces.has(node.object.name) &&
					memberName(node) === 'useCallback'
				) {
					report(node);
				}
			},
			VariableDeclarator(node) {
				trackReactAlias(node.id, node.init);
			},
			AssignmentExpression(node) {
				trackReactAlias(node.left, node.right);
			},
		};
	},
};
