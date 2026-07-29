import { basename } from 'node:path';
import { isTopLevelFunction, startLoc } from '../utils.js';

const AUTHORITY = 'See docs/frontend/react.md#tsx-file-boundary.';
const RUNTIME_DECLARATION_TYPES = new Set([
	'ClassDeclaration',
	'TSEnumDeclaration',
	'TSImportEqualsDeclaration',
	'TSModuleDeclaration',
	'VariableDeclaration',
]);
const TYPE_ONLY_DECLARATION_TYPES = new Set(['TSInterfaceDeclaration', 'TSTypeAliasDeclaration']);

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

function topLevelStatement(node) {
	let current = node;
	while (current.parent && current.parent.type !== 'Program') current = current.parent;
	return current.parent?.type === 'Program' ? current : undefined;
}

function declaredNode(statement) {
	if (
		statement.type === 'ExportDefaultDeclaration' ||
		statement.type === 'ExportNamedDeclaration'
	) {
		return statement.declaration;
	}
	return statement;
}

function declarationLocation(declaration) {
	if (declaration.type === 'VariableDeclaration') {
		return declaration.declarations[0]?.id ?? declaration;
	}
	return declaration.id ?? declaration;
}

/** @type {import('eslint').Rule.RuleModule} */
export const noExtraRuntimeDeclarationsInTsx = {
	meta: {
		type: 'suggestion',
		docs: {
			description:
				'Forbid top-level runtime declarations other than the authored TSX component function.',
		},
		schema: [],
	},
	create(context) {
		if (!shouldCheck(context.filename)) return {};

		const functionBindingStatements = new Set();

		function recordFunction(node) {
			if (!isTopLevelFunction(node)) return;
			const statement = topLevelStatement(node);
			if (statement !== undefined) functionBindingStatements.add(statement);
		}

		return {
			ArrowFunctionExpression: recordFunction,
			FunctionDeclaration: recordFunction,
			FunctionExpression: recordFunction,
			'Program:exit'(program) {
				for (const statement of program.body ?? []) {
					const declaration = declaredNode(statement);
					if (
						declaration === null ||
						declaration === undefined ||
						declaration.type === 'FunctionDeclaration' ||
						TYPE_ONLY_DECLARATION_TYPES.has(declaration.type) ||
						!RUNTIME_DECLARATION_TYPES.has(declaration.type)
					) {
						continue;
					}
					if (
						declaration.type === 'VariableDeclaration' &&
						declaration.declarations.length === 1 &&
						functionBindingStatements.has(statement)
					) {
						continue;
					}
					context.report({
						loc: startLoc(declarationLocation(declaration)),
						message:
							'Move this top-level runtime declaration to the narrowest owner outside TSX; component-owned runtime values belong in constants/, while TSX may declare only its component function and type-only TypeScript declarations. ' +
							AUTHORITY,
					});
				}
			},
		};
	},
};
