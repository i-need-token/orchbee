import { existsSync, readFileSync, readdirSync } from 'node:fs';
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
	const name = basename(filePath, extname(filePath));
	const groupDirectory = dirname(filePath);
	return (
		basename(groupDirectory) === name && basename(dirname(groupDirectory)) === COMPONENTS_DIRECTORY
	);
}

function componentName(filePath) {
	return basename(filePath, extname(filePath));
}

function isDirectBoundaryExport(filePath) {
	const indexPath = join(dirname(filePath), INDEX_FILENAME);
	if (!existsSync(indexPath)) return false;

	const name = basename(filePath, extname(filePath));
	const allowedSources = new Set([`./${name}`, `./${name}.js`, `./${name}.tsx`]);
	const indexSource = readFileSync(indexPath, 'utf8');
	const exportSources = indexSource.matchAll(/\bfrom\s*['"]([^'"]+)['"]/gu);
	return Array.from(exportSources, (match) => match[1]).some((source) =>
		allowedSources.has(source ?? ''),
	);
}

function checkDirectFiles(context, node, filePath, isPublic) {
	const directory = dirname(filePath);
	if (!existsSync(directory)) return;

	const name = componentName(filePath);
	const allowedFiles = new Set([
		...(isPublic ? [INDEX_FILENAME] : []),
		`${name}.tsx`,
		`${name}.module.css`,
	]);
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		if (entry.isDirectory() || allowedFiles.has(entry.name)) continue;
		report(
			context,
			node,
			`Move direct file "${entry.name}" into its owned support directory; component directory "${basename(directory)}" may contain only ${Array.from(allowedFiles).join(', ')} as direct files.`,
		);
	}
}

function checkPublicDirectoryName(context, node, filePath) {
	const name = componentName(filePath);
	const directoryName = basename(dirname(filePath));
	if (directoryName === name) return;
	report(
		context,
		node,
		`Rename public component directory "${directoryName}" to "${name}" so the ownership boundary matches its root component.`,
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
			description: 'Enforce public and private React component directory ownership.',
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
			const expectedSource = `./${componentName(filename)}.module.css`;
			if (
				!source.startsWith('.') ||
				normalize(dirname(resolve(dirname(filename), source))) !== normalize(dirname(filename))
			) {
				report(
					context,
					node,
					`CSS Module "${source}" must be colocated with the component that imports it.`,
				);
				return;
			}
			if (source !== expectedSource) {
				report(
					context,
					node,
					`Rename CSS Module "${source}" to "${expectedSource}" so it matches its owning component.`,
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
					} else checkDirectFiles(context, node, filename, false);
					return;
				}

				if (isDirectBoundaryExport(filename)) {
					checkPublicDirectoryName(context, node, filename);
					checkDirectFiles(context, node, filename, true);
				} else {
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
