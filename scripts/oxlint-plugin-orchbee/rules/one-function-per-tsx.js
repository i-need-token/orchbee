import { basename } from 'node:path';
import { isTopLevelFunction, startLoc } from '../utils.js';

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

		let firstFunction;
		function checkFunction(node) {
			if (!isTopLevelFunction(node)) return;
			if (firstFunction === undefined) {
				firstFunction = node;
				return;
			}

			context.report({
				loc: startLoc(node.id ?? node),
				message:
					'Move this top-level function to its own module; an authored production TSX file may contain only one top-level function. See docs/frontend/react.md#tsx-file-boundary.',
			});
		}

		return {
			ArrowFunctionExpression: checkFunction,
			FunctionDeclaration: checkFunction,
			FunctionExpression: checkFunction,
		};
	},
};
