import { existsSync, readdirSync, statSync } from 'node:fs';
import { basename, dirname, extname, join, relative, resolve } from 'node:path';
import { startLoc } from '../utils.js';

const AUTHORITY = 'See docs/MODULE-BOUNDARIES.md#module-boundaries.';
const INDEX_FILENAME = 'index.ts';
const SOURCE_EXTENSIONS = ['.ts', '.tsx', '.mts', '.cts'];
const TEST_DIRECTORY = '__tests__';
const firstSourceFileCache = new Map();

function normalize(filePath) {
	return filePath.split('\\').join('/');
}

function findSourceRoot(filePath) {
	const normalized = normalize(filePath);
	const match = normalized.match(/\/(?:apps|packages)\/[^/]+\/src(?:\/|$)/);
	if (!match || match.index === undefined) return null;
	return normalized.slice(0, match.index + match[0].length).replace(/\/$/, '');
}

function pathParts(filePath, sourceRoot) {
	const path = normalize(relative(sourceRoot, filePath));
	return path.split('/').filter((part) => part !== '' && part !== '.');
}

function moduleParts(filePath, sourceRoot) {
	const parts = pathParts(dirname(filePath), sourceRoot);
	const testDirectoryIndex = parts.indexOf(TEST_DIRECTORY);
	return testDirectoryIndex === -1 ? parts : parts.slice(0, testDirectoryIndex);
}

function isTestFile(filePath, sourceRoot) {
	return pathParts(filePath, sourceRoot).includes(TEST_DIRECTORY);
}

function isBoundaryIndex(filePath, sourceRoot) {
	return basename(filePath) === INDEX_FILENAME && !isTestFile(filePath, sourceRoot);
}

function isSourceFile(filePath) {
	return SOURCE_EXTENSIONS.includes(extname(filePath)) && !filePath.endsWith('.d.ts');
}

function firstSourceFile(directory) {
	const cached = firstSourceFileCache.get(directory);
	if (cached !== undefined) return cached;

	let result = null;
	if (existsSync(directory)) {
		for (const entry of readdirSync(directory).toSorted()) {
			if (entry === TEST_DIRECTORY || entry.startsWith('.')) continue;
			const path = join(directory, entry);
			const stats = statSync(path);
			if (stats.isFile() && isSourceFile(path)) {
				result = path;
				break;
			}
			if (!stats.isDirectory()) continue;
			const nestedSourceFile = firstSourceFile(path);
			if (nestedSourceFile !== null) {
				result = nestedSourceFile;
				break;
			}
		}
	}

	firstSourceFileCache.set(directory, result);
	return result;
}

function resolveTypeScriptSource(filename, source) {
	const unresolvedPath = resolve(dirname(filename), source);
	const extension = extname(unresolvedPath);
	const candidates = [];

	if (SOURCE_EXTENSIONS.includes(extension)) {
		candidates.push(unresolvedPath);
	} else if (extension === '.js' || extension === '.jsx') {
		const pathWithoutExtension = unresolvedPath.slice(0, -extension.length);
		candidates.push(
			...SOURCE_EXTENSIONS.map((sourceExtension) => pathWithoutExtension + sourceExtension),
		);
	} else if (extension === '') {
		candidates.push(
			...SOURCE_EXTENSIONS.map((sourceExtension) => unresolvedPath + sourceExtension),
		);
		candidates.push(join(unresolvedPath, INDEX_FILENAME));
	}

	const resolvedSource = candidates.find((candidate) => existsSync(candidate));
	return resolvedSource === undefined ? null : normalize(resolvedSource);
}

function isPrefix(prefix, value) {
	return prefix.length <= value.length && prefix.every((part, index) => part === value[index]);
}

function sameModule(left, right) {
	return left.length === right.length && isPrefix(left, right);
}

function isAncestorModule(ancestor, descendant) {
	return ancestor.length < descendant.length && isPrefix(ancestor, descendant);
}

function isVisibleModule(sourceModule, targetModule) {
	if (targetModule.length < sourceModule.length && isPrefix(targetModule, sourceModule)) {
		return true;
	}

	if (targetModule.length === 0 || targetModule.length > sourceModule.length) return false;
	const targetParent = targetModule.slice(0, -1);
	return (
		isPrefix(targetParent, sourceModule) &&
		targetModule.at(-1) !== sourceModule[targetParent.length]
	);
}

function isDirectChild(sourceModule, targetModule) {
	return targetModule.length === sourceModule.length + 1 && isPrefix(sourceModule, targetModule);
}

function moduleName(parts) {
	return parts.length === 0 ? 'src' : `src/${parts.join('/')}`;
}

function visibleTargetBoundary(sourceModule, targetModule) {
	for (let depth = targetModule.length - 1; depth > 0; depth -= 1) {
		const candidate = targetModule.slice(0, depth);
		if (isVisibleModule(sourceModule, candidate))
			return `${moduleName(candidate)}/${INDEX_FILENAME}`;
	}
	return null;
}

function report(context, node, message) {
	context.report({
		loc: startLoc(node.source ?? node),
		message: `${message} ${AUTHORITY}`,
	});
}

function reportMissingIndexes(context, program, filename, sourceRoot, sourceModule) {
	for (let depth = 0; depth <= sourceModule.length; depth += 1) {
		const directory = join(sourceRoot, ...sourceModule.slice(0, depth));
		const indexPath = join(directory, INDEX_FILENAME);
		const firstFile = firstSourceFile(directory);
		if (existsSync(indexPath) || firstFile === null || normalize(firstFile) !== filename) continue;
		report(
			context,
			program,
			`Source module "${moduleName(sourceModule.slice(0, depth))}" must define ${INDEX_FILENAME} as its boundary entry point.`,
		);
	}
}

function reportInvalidIndex(context, program, filename, sourceRoot) {
	if (!isBoundaryIndex(filename, sourceRoot)) return;
	const invalidStatement = program.body?.find(
		(statement) =>
			statement.type !== 'ExportNamedDeclaration' ||
			statement.source === null ||
			statement.source === undefined ||
			(statement.declaration !== null && statement.declaration !== undefined),
	);
	if (!invalidStatement) return;
	report(
		context,
		invalidStatement,
		`${normalize(relative(sourceRoot, filename))} may contain only explicit named re-exports with a source; imports, declarations, statements, export *, and local exports are forbidden.`,
	);
}

/** @type {import('eslint').Rule.RuleModule} */
export const moduleBoundaries = {
	meta: {
		type: 'problem',
		docs: {
			description: 'Enforce hierarchical source-module visibility and explicit boundary entries.',
		},
		schema: [],
	},
	create(context) {
		const filename = normalize(context.filename);
		const sourceRoot = findSourceRoot(filename);
		if (sourceRoot === null) return {};
		const sourceModule = moduleParts(filename, sourceRoot);
		const sourceIsBoundaryIndex = isBoundaryIndex(filename, sourceRoot);
		const sourceIsRootTest = sourceModule.length === 0 && isTestFile(filename, sourceRoot);

		function checkProgram(program) {
			reportMissingIndexes(context, program, filename, sourceRoot, sourceModule);
			reportInvalidIndex(context, program, filename, sourceRoot);
		}

		function checkSource(node) {
			const source = node.source?.value;
			if (typeof source !== 'string' || !source.startsWith('.')) return;
			if (sourceIsBoundaryIndex && node.type !== 'ExportNamedDeclaration') return;

			const targetFile = resolveTypeScriptSource(filename, source);
			if (targetFile === null || findSourceRoot(targetFile) !== sourceRoot) return;
			const targetModule = moduleParts(targetFile, sourceRoot);

			if (!isTestFile(filename, sourceRoot) && isTestFile(targetFile, sourceRoot)) {
				report(context, node, `Production source must not import test source "${source}".`);
				return;
			}

			if (sameModule(sourceModule, targetModule)) {
				if (targetFile !== filename && isBoundaryIndex(targetFile, sourceRoot)) {
					report(
						context,
						node,
						`Source inside "${moduleName(sourceModule)}" must import its implementation files directly instead of importing its own ${INDEX_FILENAME}.`,
					);
				}
				return;
			}

			if (isAncestorModule(targetModule, sourceModule)) return;

			const targetIsBoundaryIndex = isBoundaryIndex(targetFile, sourceRoot);
			if (!targetIsBoundaryIndex) {
				report(
					context,
					node,
					`Dependency "${source}" bypasses the "${moduleName(targetModule)}" boundary; import its ${INDEX_FILENAME} instead.`,
				);
				return;
			}

			if (sourceIsRootTest) return;

			if (
				sourceIsBoundaryIndex &&
				node.type === 'ExportNamedDeclaration' &&
				isDirectChild(sourceModule, targetModule)
			) {
				return;
			}

			if (isVisibleModule(sourceModule, targetModule)) return;
			const targetBoundary = visibleTargetBoundary(sourceModule, targetModule);
			const action = targetBoundary
				? `Explicitly re-export the required API through "${targetBoundary}" and import that visible boundary`
				: 'Move the dependency into a sibling composition module or another visible boundary';
			report(
				context,
				node,
				`Module "${moduleName(sourceModule)}" cannot depend on hidden module "${moduleName(targetModule)}". ${action}.`,
			);
		}

		return {
			Program: checkProgram,
			ImportDeclaration: checkSource,
			ImportExpression: checkSource,
			ExportAllDeclaration: checkSource,
			ExportNamedDeclaration: checkSource,
		};
	},
};
