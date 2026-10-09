import { existsSync, readFileSync } from 'node:fs';
import { basename, dirname, extname, join, resolve } from 'node:path';
import { startLoc } from '../utils.js';

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
		/^[A-Z][A-Za-z0-9]*\.tsx$/u.test(fileName) &&
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

function kebabCase(value) {
	return value
		.replace(/([A-Z]+)([A-Z][a-z])/gu, '$1-$2')
		.replace(/([a-z0-9])([A-Z])/gu, '$1-$2')
		.toLowerCase();
}

function hasNamedComponentGroup(filePath) {
	const name = basename(filePath, extname(filePath));
	const groupDirectory = dirname(filePath);
	return (
		basename(groupDirectory) === kebabCase(name) &&
		basename(dirname(groupDirectory)) === COMPONENTS_DIRECTORY
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

function checkPrivateIndex(context, node, filePath) {
	if (existsSync(join(dirname(filePath), INDEX_FILENAME))) {
		report(
			context,
			node,
			'Remove the private component index.ts barrel; expose public API through the owning source module index.ts.',
		);
	}
}

function checkPublicDirectoryName(context, node, filePath) {
	const name = componentName(filePath);
	const directoryName = basename(dirname(filePath));
	const expectedDirectoryName = kebabCase(name);
	if (directoryName === expectedDirectoryName) return;
	report(
		context,
		node,
		`Rename public component directory "${directoryName}" to "${expectedDirectoryName}" so the kebab-case ownership boundary matches component "${name}".`,
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
			ImportDeclaration: checkStyleImport,
			'Program:exit'(node) {
				if (isInsideComponentsDirectory(filename)) {
					if (!hasNamedComponentGroup(filename)) {
						report(
							context,
							node,
							'Place this component in its own components/<component-name>/<ComponentName>.tsx group.',
						);
					} else checkPrivateIndex(context, node, filename);
					return;
				}

				if (isDirectBoundaryExport(filename)) {
					checkPublicDirectoryName(context, node, filename);
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
