import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { filenameNamingConvention } from '../filename-naming-convention';
import { moduleBoundaries } from '../module-boundaries';
import { noCrossWorkspaceSourceImports } from '../no-cross-workspace-source-imports';
import { noInlineSuppressions } from '../no-inline-suppressions';
import { requireUnitTest } from '../require-unit-test';

interface LintReport {
	readonly message: string;
}

interface CommentNode {
	readonly value: string;
	readonly loc: {
		readonly start: { readonly line: number; readonly column: number };
	};
}

function createContext(filename: string, comments: readonly CommentNode[] = []) {
	const reports: LintReport[] = [];
	return {
		reports,
		context: {
			filename,
			sourceCode: {
				getAllComments: () => comments,
			},
			report: (report: LintReport): void => {
				reports.push(report);
			},
		},
	};
}

const LOCATION = {
	start: { line: 1, column: 0 },
	end: { line: 1, column: 1 },
};

const fixtureRoots = new Set<string>();

function createFixture() {
	const root = mkdtempSync(join(tmpdir(), 'orchbee-module-boundaries-'));
	fixtureRoots.add(root);
	return root;
}

function writeFixture(root: string, path: string) {
	const filename = join(root, path);
	mkdirSync(dirname(filename), { recursive: true });
	writeFileSync(filename, '');
	return filename;
}

function createModuleTree() {
	const root = createFixture();
	for (const path of [
		'packages/example/src/index.ts',
		'packages/example/src/a/index.ts',
		'packages/example/src/a/a1/index.ts',
		'packages/example/src/a/a1/internal.ts',
		'packages/example/src/b/index.ts',
		'packages/example/src/b/b1/index.ts',
		'packages/example/src/b/b1/consumer.ts',
	]) {
		writeFixture(root, path);
	}
	return root;
}

function dependencyNode(type: string, source: string) {
	return {
		type,
		loc: LOCATION,
		source: { value: source, loc: LOCATION },
	};
}

afterEach(() => {
	for (const root of fixtureRoots) rmSync(root, { recursive: true, force: true });
	fixtureRoots.clear();
});

describe('filename-naming-convention', () => {
	it('accepts sibling TSX unit tests with kebab-case paths', () => {
		const { context, reports } = createContext(
			'/workspace/packages/ui/src/components/__tests__/agent-panel.unit.test.tsx',
		);
		filenameNamingConvention.create(context).Program({ loc: LOCATION });
		expect(reports).toEqual([]);
	});

	it('reports the extension-preserving TSX unit-test suffix', () => {
		const { context, reports } = createContext(
			'/workspace/packages/ui/src/components/__tests__/agent-panel.test.tsx',
		);
		filenameNamingConvention.create(context).Program({ loc: LOCATION });
		expect(reports[0]?.message).toContain('<source-name>.unit.test.tsx');
	});
});

describe('no-cross-workspace-source-imports', () => {
	it('rejects re-exports from another package source tree', () => {
		const { context, reports } = createContext('/workspace/apps/desktop/src/terminal.ts');
		noCrossWorkspaceSourceImports.create(context).ExportNamedDeclaration({
			source: {
				value: '../../../packages/terminal/src/terminal.js',
				loc: LOCATION,
			},
		});
		expect(reports[0]?.message).toContain('Import "@orchbee/terminal"');
	});

	it('accepts package public exports', () => {
		const { context, reports } = createContext('/workspace/apps/desktop/src/terminal.ts');
		noCrossWorkspaceSourceImports.create(context).ImportDeclaration({
			source: { value: '@orchbee/terminal', loc: LOCATION },
		});
		expect(reports).toEqual([]);
	});
});

describe('module-boundaries', () => {
	it('allows B1 to import its ancestor sibling A through A index', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/b/b1/consumer.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ImportDeclaration(dependencyNode('ImportDeclaration', '../../a'));
		expect(reports).toEqual([]);
	});

	it('hides A1 from B1 even when B1 targets the A1 index', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/b/b1/consumer.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ImportDeclaration(dependencyNode('ImportDeclaration', '../../a/a1'));
		expect(reports[0]?.message).toContain('cannot depend on hidden module "src/a/a1"');
		expect(reports[0]?.message).toContain('src/a/index.ts');
	});

	it('allows A index to explicitly promote API from its direct child A1 index', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/a/index.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ExportNamedDeclaration(dependencyNode('ExportNamedDeclaration', './a1'));
		expect(reports).toEqual([]);
	});

	it('rejects direct access to a private file in another module', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/b/b1/consumer.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ImportDeclaration(dependencyNode('ImportDeclaration', '../../a/a1/internal'));
		expect(reports[0]?.message).toContain('bypasses the "src/a/a1" boundary');
	});

	it('rejects a parent importing its child index', () => {
		const root = createModuleTree();
		const filename = writeFixture(root, 'packages/example/src/b/parent.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries.create(context).ImportDeclaration(dependencyNode('ImportDeclaration', './b1'));
		expect(reports[0]?.message).toContain('cannot depend on hidden module "src/b/b1"');
	});

	it('requires an index for every source module', () => {
		const root = createFixture();
		writeFixture(root, 'packages/example/src/index.ts');
		const filename = writeFixture(root, 'packages/example/src/missing/implementation.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries.create(context).Program({ type: 'Program', loc: LOCATION, body: [] });
		expect(reports[0]?.message).toContain('"src/missing" must define index.ts');
	});

	it('keeps boundary indexes export-only and explicit', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/a/index.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries.create(context).Program({
			type: 'Program',
			loc: LOCATION,
			body: [{ type: 'ExportAllDeclaration', loc: LOCATION }],
		});
		expect(reports[0]?.message).toContain('only explicit named re-exports');
	});
});

describe('no-inline-suppressions', () => {
	it('rejects lint suppressions but leaves TypeScript directives to ban-ts-comment', () => {
		const comments = [
			{ value: ' eslint-disable-next-line no-console', loc: LOCATION },
			{ value: ' @ts-expect-error -- verifies invalid input', loc: LOCATION },
		];
		const { context, reports } = createContext('/workspace/example.unit.test.ts', comments);
		noInlineSuppressions.create(context).Program();
		expect(reports).toHaveLength(1);
		expect(reports[0]?.message).toContain('eslint-disable');
	});
});

describe('require-unit-test', () => {
	it('requires a sibling TSX test for a top-level arrow function', () => {
		const { context, reports } = createContext(
			'/workspace/packages/ui/src/components/agent-panel.tsx',
		);
		const program = { type: 'Program' };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const variableDeclaration = { type: 'VariableDeclaration', parent: exportDeclaration };
		requireUnitTest.create(context).VariableDeclarator({
			parent: variableDeclaration,
			init: { type: 'ArrowFunctionExpression' },
			id: { loc: LOCATION },
		});
		expect(reports[0]?.message).toContain('agent-panel.unit.test.tsx');
	});

	it('does not require tests for type-only source files', () => {
		const { context, reports } = createContext('/workspace/packages/core/src/types.ts');
		const visitors = requireUnitTest.create(context);
		expect(visitors).not.toHaveProperty('Program');
		expect(reports).toEqual([]);
	});
});
