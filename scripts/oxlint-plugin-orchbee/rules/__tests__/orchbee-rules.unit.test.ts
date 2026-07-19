import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { componentDirectory } from '../component-directory';
import { filenameNamingConvention } from '../filename-naming-convention';
import { moduleBoundaries } from '../module-boundaries';
import { noCrossWorkspaceSourceImports } from '../no-cross-workspace-source-imports';
import { noInlineSuppressions } from '../no-inline-suppressions';
import { noReactHooksInTsx } from '../no-react-hooks-in-tsx';
import { oneFunctionPerTsx } from '../one-function-per-tsx';
import { oneHookPerFile } from '../one-hook-per-file';
import { requireUnitTest } from '../require-unit-test';
import { zustandScope } from '../zustand-scope';

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

function writeFixture(root: string, path: string, source = '') {
	const filename = join(root, path);
	mkdirSync(dirname(filename), { recursive: true });
	writeFileSync(filename, source);
	return filename;
}

function createModuleTree() {
	const root = createFixture();
	for (const path of [
		'packages/example/src/index.ts',
		'packages/example/src/root-utility.ts',
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

	it('allows a child to import an ancestor implementation directly', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/b/b1/consumer.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ImportDeclaration(dependencyNode('ImportDeclaration', '../../root-utility'));
		expect(reports).toEqual([]);
	});

	it('allows package-level tests to use descendant module indexes', () => {
		const root = createModuleTree();
		const filename = join(root, 'packages/example/src/__tests__/integration.unit.test.ts');
		const { context, reports } = createContext(filename);
		moduleBoundaries
			.create(context)
			.ImportDeclaration(dependencyNode('ImportDeclaration', '../a/a1'));
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

	it('treats named component groups as structural directories owned by their module', () => {
		const root = createFixture();
		writeFixture(root, 'packages/example/src/index.ts');
		writeFixture(root, 'packages/example/src/composition/index.ts');
		const filename = writeFixture(root, 'packages/example/src/composition/workbench.tsx');
		writeFixture(root, 'packages/example/src/composition/components/toolbar/toolbar.tsx');
		const { context, reports } = createContext(filename);
		const visitors = moduleBoundaries.create(context);

		visitors.Program({ type: 'Program', loc: LOCATION, body: [] });
		visitors.ImportDeclaration(dependencyNode('ImportDeclaration', './components/toolbar/toolbar'));

		expect(reports).toEqual([]);
	});

	it('keeps a generic directory named components as a source-module boundary', () => {
		const root = createFixture();
		writeFixture(root, 'packages/example/src/index.ts');
		writeFixture(root, 'packages/example/src/components/index.ts');
		writeFixture(root, 'packages/example/src/components/ui/index.ts');
		const filename = writeFixture(root, 'packages/example/src/components/consumer.ts');
		const { context, reports } = createContext(filename);

		moduleBoundaries.create(context).ImportDeclaration(dependencyNode('ImportDeclaration', './ui'));

		expect(reports[0]?.message).toContain('cannot depend on hidden module "src/components/ui"');
	});
});

describe('component-directory', () => {
	it('accepts a module boundary component re-exported by its sibling index', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/workbench.tsx');
		writeFixture(
			root,
			'packages/example/src/composition/index.ts',
			"export { Workbench } from './workbench';\n",
		);
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Workbench', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports).toEqual([]);
	});

	it('rejects a private child component left loose in its source module', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/workbench-toolbar.tsx');
		writeFixture(root, 'packages/example/src/composition/index.ts');
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'WorkbenchToolbar', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('private child component');
		expect(reports[0]?.message).toContain('docs/frontend/react.md#component-directory-boundary');
	});

	it('accepts recursively named private component groups', () => {
		const root = createFixture();
		const filename = writeFixture(
			root,
			'packages/example/src/composition/components/toolbar/components/action/action.tsx',
		);
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Action', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports).toEqual([]);
	});

	it('rejects a component placed loose directly under components', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/components/toolbar.tsx');
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Toolbar', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('components/<component-name>/<component-name>.tsx');
	});

	it('rejects a CSS Module imported from outside the component group', () => {
		const { context, reports } = createContext(
			'/workspace/packages/example/src/composition/components/toolbar/toolbar.tsx',
		);

		componentDirectory.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			loc: LOCATION,
			source: { value: '../../toolbar.module.css', loc: LOCATION },
		});

		expect(reports[0]?.message).toContain('must be colocated');
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

describe('no-react-hooks-in-tsx', () => {
	it('rejects direct React namespace Hook calls in authored TSX', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench.tsx',
		);
		const visitors = noReactHooksInTsx.create(context);
		visitors.ImportDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ImportNamespaceSpecifier',
					local: { type: 'Identifier', name: 'React' },
				},
			],
		});
		visitors.CallExpression({
			callee: {
				type: 'MemberExpression',
				computed: false,
				object: { type: 'Identifier', name: 'React' },
				property: { type: 'Identifier', name: 'useEffect' },
				loc: LOCATION,
			},
		});

		expect(reports[0]?.message).toContain('React.useEffect');
		expect(reports[0]?.message).toContain('docs/frontend/react.md#component-hook-boundary');
	});

	it('rejects directly imported React Hooks in authored TSX', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench.tsx',
		);
		const visitors = noReactHooksInTsx.create(context);
		visitors.ImportDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					imported: { type: 'Identifier', name: 'useState' },
					local: { type: 'Identifier', name: 'useFeatureState' },
				},
			],
		});
		visitors.CallExpression({
			callee: { type: 'Identifier', name: 'useFeatureState', loc: LOCATION },
		});

		expect(reports[0]?.message).toContain('useFeatureState');
	});

	it('accepts responsibility-focused custom Hooks in authored TSX', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench.tsx',
		);
		noReactHooksInTsx.create(context).CallExpression({
			callee: { type: 'Identifier', name: 'useWorkbenchProps', loc: LOCATION },
		});

		expect(reports).toEqual([]);
	});
});

describe('one-function-per-tsx', () => {
	it('reports every additional top-level function in authored production TSX', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench.tsx',
		);
		const program = { type: 'Program' };
		const firstFunction = {
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Workbench', loc: LOCATION },
		};
		const secondFunction = {
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'WorkbenchToolbar', loc: LOCATION },
		};
		const visitors = oneFunctionPerTsx.create(context);

		visitors.FunctionDeclaration(firstFunction);
		visitors.FunctionDeclaration(secondFunction);

		expect(reports).toHaveLength(1);
		expect(reports[0]?.message).toContain('docs/frontend/react.md#tsx-file-boundary');
	});

	it('accepts callbacks nested inside the sole TSX component', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench.tsx',
		);
		const program = { type: 'Program' };
		const component = {
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Workbench', loc: LOCATION },
		};
		const nestedCallback = {
			type: 'ArrowFunctionExpression',
			parent: component,
			loc: LOCATION,
		};
		const visitors = oneFunctionPerTsx.create(context);

		visitors.FunctionDeclaration(component);
		visitors.ArrowFunctionExpression(nestedCallback);

		expect(reports).toEqual([]);
	});
});

describe('one-hook-per-file', () => {
	it('accepts one named Hook function exported from its module', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/use-workbench-view-props.ts',
		);
		const program = { type: 'Program', loc: LOCATION };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const hook = {
			type: 'FunctionDeclaration',
			parent: exportDeclaration,
			id: { type: 'Identifier', name: 'useWorkbenchViewProps', loc: LOCATION },
		};
		const visitors = oneHookPerFile.create(context);

		visitors.FunctionDeclaration(hook);
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: hook,
			specifiers: [],
		});
		visitors['Program:exit'](program);

		expect(reports).toEqual([]);
	});

	it('rejects a second top-level function in a Hook module', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/use-workbench-view-props.ts',
		);
		const program = { type: 'Program', loc: LOCATION };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const hook = {
			type: 'FunctionDeclaration',
			parent: exportDeclaration,
			id: { type: 'Identifier', name: 'useWorkbenchViewProps', loc: LOCATION },
		};
		const helper = {
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'prepareTabs', loc: LOCATION },
		};
		const visitors = oneHookPerFile.create(context);

		visitors.FunctionDeclaration(hook);
		visitors.FunctionDeclaration(helper);
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: hook,
			specifiers: [],
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('wrong number of functions');
	});

	it('counts type exports against the Hook module export boundary', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/use-workbench-view-props.ts',
		);
		const program = { type: 'Program', loc: LOCATION };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const hook = {
			type: 'FunctionDeclaration',
			parent: exportDeclaration,
			id: { type: 'Identifier', name: 'useWorkbenchViewProps', loc: LOCATION },
		};
		const visitors = oneHookPerFile.create(context);

		visitors.FunctionDeclaration(hook);
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: hook,
			specifiers: [],
		});
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: {
				type: 'TSInterfaceDeclaration',
				id: { type: 'Identifier', name: 'WorkbenchViewProps', loc: LOCATION },
			},
			specifiers: [],
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('wrong number of exports');
	});
});

describe('zustand-scope', () => {
	it('rejects bound Store creation from the React Zustand entry', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/shell/workbench-store.ts',
		);
		zustandScope.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'create' },
					local: { type: 'Identifier', name: 'create', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});

		expect(reports[0]?.message).toContain('process-wide singleton');
	});

	it('rejects direct useStore imports in component modules', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/composition/workbench-content.tsx',
		);
		zustandScope.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'useStore' },
					local: { type: 'Identifier', name: 'useStore', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});

		expect(reports[0]?.message).toContain('contexts/*-store');
	});

	it('accepts a selector subscription inside a component Context Store module', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/contexts/workbench-store.ts',
		);
		const visitors = zustandScope.create(context);
		visitors.ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'useStore' },
					local: { type: 'Identifier', name: 'useStore', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});
		visitors.CallExpression({
			type: 'CallExpression',
			arguments: [{}, {}],
			callee: { type: 'Identifier', name: 'useStore', loc: LOCATION },
			loc: LOCATION,
		});

		expect(reports).toEqual([]);
	});

	it('rejects a forwarding Store selector Hook in the hooks directory', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/hooks/use-workbench-store.ts',
		);
		zustandScope.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'useStore' },
					local: { type: 'Identifier', name: 'useStore', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});

		expect(reports[0]?.message).toContain('contexts/*-store');
	});

	it('requires an explicit selector for useStore subscriptions', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/contexts/workbench-store.ts',
		);
		const visitors = zustandScope.create(context);
		visitors.ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'useStore' },
					local: { type: 'Identifier', name: 'useStore', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});
		visitors.CallExpression({
			type: 'CallExpression',
			arguments: [{}],
			callee: { type: 'Identifier', name: 'useStore', loc: LOCATION },
			loc: LOCATION,
		});

		expect(reports[0]?.message).toContain('explicit selector');
	});

	it('rejects module-level vanilla Store instances', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/shell/workbench-store.ts',
		);
		const visitors = zustandScope.create(context);
		visitors.ImportDeclaration({
			type: 'ImportDeclaration',
			importKind: 'value',
			loc: LOCATION,
			source: { value: 'zustand/vanilla' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					importKind: 'value',
					imported: { type: 'Identifier', name: 'createStore' },
					local: { type: 'Identifier', name: 'createStore', loc: LOCATION },
					loc: LOCATION,
				},
			],
		});
		visitors.CallExpression({
			type: 'CallExpression',
			arguments: [],
			callee: { type: 'Identifier', name: 'createStore', loc: LOCATION },
			loc: LOCATION,
			parent: { type: 'VariableDeclarator', parent: { type: 'Program' } },
		});

		expect(reports[0]?.message).toContain('shared across feature mounts');
	});
});

describe('require-unit-test', () => {
	it('requires a sibling test for a top-level arrow function in a TS module', () => {
		const { context, reports } = createContext('/workspace/packages/core/src/session-service.ts');
		const program = { type: 'Program' };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const variableDeclaration = { type: 'VariableDeclaration', parent: exportDeclaration };
		requireUnitTest.create(context).VariableDeclarator({
			parent: variableDeclaration,
			init: { type: 'ArrowFunctionExpression' },
			id: { loc: LOCATION },
		});
		expect(reports[0]?.message).toContain('session-service.unit.test.ts');
	});

	it('does not require sibling tests for TSX components', () => {
		const { context, reports } = createContext(
			'/workspace/packages/ui/src/components/agent-panel.tsx',
		);
		const visitors = requireUnitTest.create(context);
		expect(visitors).not.toHaveProperty('VariableDeclarator');
		expect(reports).toEqual([]);
	});

	it('does not require tests for type-only source files', () => {
		const { context, reports } = createContext('/workspace/packages/core/src/types.ts');
		const visitors = requireUnitTest.create(context);
		expect(visitors).not.toHaveProperty('Program');
		expect(reports).toEqual([]);
	});
});
