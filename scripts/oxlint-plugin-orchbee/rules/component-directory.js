import { existsSync, readFileSync } from 'node:fs';
import { basename, dirname, extname, join, resolve } from 'node:path';
import { isTopLevelFunction, startLoc } from '../utils.js';

const AUTHORITY = 'See docs/frontend/react.md#component-directory-boundary.';
const COMPONENTS_DIRECTORY = 'components';
const INDEX_FILENAME = 'index.ts';

function normalize(filePath) {
	return filePath.split('\\').join('/');
}

function shouldCheck(filePath) {
	const normalized = normalize(filePath);
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

function sourceParts(filePath) {
	const normalized = normalize(filePath);
	const sourceMarker = '/src/';
	const sourceIndex = normalized.indexOf(sourceMarker);
	if (sourceIndex === -1) return [];
	return normalized
		.slice(sourceIndex + sourceMarker.length)
		.split('/')
		.filter((part) => part !== '');
}

function isInsideComponentsDirectory(filePath) {
	return sourceParts(dirname(filePath)).includes(COMPONENTS_DIRECTORY);
}

function hasNamedComponentGroup(filePath) {
	const componentName = basename(filePath, extname(filePath));
	const groupDirectory = dirname(filePath);
	return (
		basename(groupDirectory) === componentName &&
		basename(dirname(groupDirectory)) === COMPONENTS_DIRECTORY
	);
}

function isDirectBoundaryExport(filePath) {
	const indexPath = join(dirname(filePath), INDEX_FILENAME);
	if (!existsSync(indexPath)) return false;

	const componentName = basename(filePath, extname(filePath));
	const allowedSources = new Set([
		`./${componentName}`,
		`./${componentName}.js`,
		`./${componentName}.tsx`,
	]);
	const indexSource = readFileSync(indexPath, 'utf8');
	const exportSources = indexSource.matchAll(/\bfrom\s*['"]([^'"]+)['"]/gu);
	return Array.from(exportSources, (match) => match[1]).some((source) =>
		allowedSources.has(source ?? ''),
	);
}

function report(context, node, message) {
	context.report({
		loc: startLoc(node.source ?? node),
		message: `${message} ${AUTHORITY}`,
	});
}

/** @type {import('eslint').Rule.RuleModule} */
export const componentDirectory = {
	meta: {
		type: 'suggestion',
		docs: {
			description: 'Keep private React components and their styles in named component groups.',
		},
		schema: [],
	},
	create(context) {
		const filename = normalize(context.filename);
		if (!shouldCheck(filename)) return {};

		let hasTopLevelFunction = false;

		function recordFunction(node) {
			if (isTopLevelFunction(node)) hasTopLevelFunction = true;
		}

		function checkStyleImport(node) {
			const source = node.source?.value;
			if (typeof source !== 'string' || !source.endsWith('.module.css')) return;
			if (
				!source.startsWith('.') ||
				normalize(dirname(resolve(dirname(filename), source))) !== normalize(dirname(filename))
			) {
				report(
					context,
					node,
					`CSS Module "${source}" must be colocated with the component that imports it.`,
				);
			}
		}

		return {
			ArrowFunctionExpression: recordFunction,
			FunctionDeclaration: recordFunction,
			FunctionExpression: recordFunction,
			ImportDeclaration: checkStyleImport,
			'Program:exit'(node) {
				if (!hasTopLevelFunction) return;
				if (isInsideComponentsDirectory(filename)) {
					if (!hasNamedComponentGroup(filename)) {
						report(
							context,
							node,
							'Place this component in its own components/<component-name>/<component-name>.tsx group.',
						);
					}
					return;
				}

				if (!isDirectBoundaryExport(filename)) {
					report(
						context,
						node,
						'Move this private child component into a named components/<component-name>/ group.',
					);
				}
			},
		};
	},
};
