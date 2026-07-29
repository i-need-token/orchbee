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
import { noReactUseCallback } from '../no-react-use-callback';
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

function lintFilename(filename: string, body: readonly object[] = []) {
	const { context, reports } = createContext(filename);
	const program = { type: 'Program', body, loc: LOCATION };
	const listeners = filenameNamingConvention.create(context);
	listeners.Program?.(program);
	listeners['Program:exit']?.(program);
	return reports;
}

function exportedFunction(name: string) {
	return {
		type: 'ExportNamedDeclaration',
		exportKind: 'value',
		declaration: {
			type: 'FunctionDeclaration',
			id: { type: 'Identifier', name },
		},
		specifiers: [],
	};
}

function exportedClass(name: string) {
	return {
		type: 'ExportNamedDeclaration',
		exportKind: 'value',
		declaration: {
			type: 'ClassDeclaration',
			id: { type: 'Identifier', name },
		},
		specifiers: [],
	};
}

function exportedInterface(name: string) {
	return {
		type: 'ExportNamedDeclaration',
		exportKind: 'type',
		declaration: {
			type: 'TSInterfaceDeclaration',
			id: { type: 'Identifier', name },
		},
		specifiers: [],
	};
}

function exportedConstant(name: string) {
	return {
		type: 'ExportNamedDeclaration',
		exportKind: 'value',
		declaration: {
			type: 'VariableDeclaration',
			declarations: [
				{
					type: 'VariableDeclarator',
					id: { type: 'Identifier', name },
					init: { type: 'ObjectExpression' },
				},
			],
		},
		specifiers: [],
	};
}

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
		const reports = lintFilename(
			'/workspace/packages/ui/src/components/__tests__/agent-panel.unit.test.tsx',
		);
		expect(reports).toEqual([]);
	});

	it('accepts symbol-matched component and Hook source paths', () => {
		expect(
			lintFilename('/workspace/packages/workbench/src/terminal-pane/TerminalPane.tsx'),
		).toEqual([]);
		expect(
			lintFilename('/workspace/packages/workbench/src/terminal-pane/hooks/useTerminalPane.ts', [
				exportedFunction('useTerminalPane'),
			]),
		).toEqual([]);
		expect(
			lintFilename(
				'/workspace/packages/workbench/src/terminal-pane/__tests__/TerminalPane.unit.test.tsx',
			),
		).toEqual([]);
		expect(
			lintFilename(
				'/workspace/packages/workbench/src/terminal-pane/hooks/__tests__/useTerminalPane.unit.test.ts',
			),
		).toEqual([]);
	});

	it('accepts symbol-matched function and class source paths', () => {
		expect(
			lintFilename('/workspace/apps/web/src/composition/startWebApplication.ts', [
				exportedFunction('startWebApplication'),
			]),
		).toEqual([]);
		expect(
			lintFilename('/workspace/packages/terminal/src/EventEmitter.ts', [
				exportedInterface('Disposable'),
				exportedClass('EventEmitter'),
			]),
		).toEqual([]);
		expect(
			lintFilename(
				'/workspace/apps/web/src/composition/__tests__/startWebApplication.unit.test.ts',
			),
		).toEqual([]);
		expect(
			lintFilename('/workspace/packages/terminal/src/__tests__/EventEmitter.unit.test.ts'),
		).toEqual([]);
	});

	it('requires a sole runtime function or class export to match the source filename', () => {
		const functionReports = lintFilename(
			'/workspace/apps/web/src/composition/start-web-application.ts',
			[exportedFunction('startWebApplication')],
		);
		expect(functionReports[0]?.message).toContain('startWebApplication.ts');

		const classReports = lintFilename('/workspace/packages/terminal/src/event-emitter.ts', [
			exportedClass('EventEmitter'),
		]);
		expect(classReports[0]?.message).toContain('EventEmitter.ts');
	});

	it('keeps responsibility-focused names for grouped runtime exports and data modules', () => {
		expect(
			lintFilename('/workspace/packages/terminal/src/font-metrics.ts', [
				exportedFunction('parseFontMetricModifier'),
				exportedFunction('applyFontMetricModifier'),
			]),
		).toEqual([]);
		expect(
			lintFilename('/workspace/packages/i18n/src/locales/en-US/web.ts', [
				exportedConstant('enUSWebMessages'),
			]),
		).toEqual([]);
	});

	it('preserves the upstream filename for generated shadcn source', () => {
		expect(
			lintFilename('/workspace/packages/ui/src/hooks/use-mobile.ts', [
				exportedFunction('useIsMobile'),
			]),
		).toEqual([]);
	});

	it('accepts generator-owned kebab-case filenames', () => {
		expect(
			lintFilename('/workspace/packages/terminal/src/nerd-font-constraints.generated.ts', [
				exportedFunction('buildNerdFontConstraints'),
			]),
		).toEqual([]);
	});

	it('rejects camelCase ordinary source paths', () => {
		const reports = lintFilename(
			'/workspace/packages/workbench/src/terminal-pane/terminalPaneState.ts',
		);
		expect(reports[0]?.message).toContain('kebab-case');
	});

	it('accepts canonical BCP 47 directories directly under locales', () => {
		for (const locale of ['en-US', 'zh-CN']) {
			const reports = lintFilename(`/workspace/packages/i18n/src/locales/${locale}/workbench.ts`);
			expect(reports).toEqual([]);
		}
	});

	it('reports non-canonical locale directory casing', () => {
		const reports = lintFilename('/workspace/packages/i18n/src/locales/en-us/workbench.ts');
		expect(reports[0]?.message).toContain('"en-US"');
	});

	it('reports the extension-preserving TSX unit-test suffix', () => {
		const reports = lintFilename(
			'/workspace/packages/ui/src/components/__tests__/agent-panel.test.tsx',
		);
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
		const filename = writeFixture(root, 'packages/example/src/composition/Workbench.tsx');
		writeFixture(root, 'packages/example/src/composition/components/toolbar/Toolbar.tsx');
		const { context, reports } = createContext(filename);
		const visitors = moduleBoundaries.create(context);

		visitors.Program({ type: 'Program', loc: LOCATION, body: [] });
		visitors.ImportDeclaration(dependencyNode('ImportDeclaration', './components/toolbar/Toolbar'));

		expect(reports).toEqual([]);
	});

	it('preserves component acronym casing while matching a kebab-case ownership directory', () => {
		const root = createFixture();
		writeFixture(root, 'packages/example/src/index.ts');
		writeFixture(root, 'packages/example/src/composition/index.ts');
		const filename = writeFixture(root, 'packages/example/src/composition/Workbench.tsx');
		writeFixture(root, 'packages/example/src/composition/components/html-viewer/HTMLViewer.tsx');
		const { context, reports } = createContext(filename);

		moduleBoundaries
			.create(context)
			.ImportDeclaration(
				dependencyNode('ImportDeclaration', './components/html-viewer/HTMLViewer'),
			);

		expect(reports).toEqual([]);
	});

	it('treats public component support directories as structural parts of their module', () => {
		const root = createFixture();
		writeFixture(root, 'packages/example/src/index.ts');
		writeFixture(
			root,
			'packages/example/src/workbench/index.ts',
			"export { Workbench } from './Workbench';\n",
		);
		const filename = writeFixture(root, 'packages/example/src/workbench/Workbench.tsx');
		writeFixture(root, 'packages/example/src/workbench/hooks/useWorkbench.ts');
		const { context, reports } = createContext(filename);
		const visitors = moduleBoundaries.create(context);

		visitors.Program({ type: 'Program', loc: LOCATION, body: [] });
		visitors.ImportDeclaration(dependencyNode('ImportDeclaration', './hooks/useWorkbench'));

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
		const filename = writeFixture(root, 'packages/example/src/workbench/Workbench.tsx');
		writeFixture(
			root,
			'packages/example/src/workbench/index.ts',
			"export { Workbench } from './Workbench';\n",
		);
		writeFixture(root, 'packages/example/src/workbench/Workbench.module.css');
		writeFixture(root, 'packages/example/src/workbench/hooks/useWorkbench.ts');
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

	it('matches a PascalCase acronym component to its kebab-case public directory', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/html-viewer/HTMLViewer.tsx');
		writeFixture(
			root,
			'packages/example/src/html-viewer/index.ts',
			"export { HTMLViewer } from './HTMLViewer';\n",
		);
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'HTMLViewer', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports).toEqual([]);
	});

	it('rejects a public component directory whose name does not match its component', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/Workbench.tsx');
		writeFixture(
			root,
			'packages/example/src/composition/index.ts',
			"export { Workbench } from './Workbench';\n",
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

		expect(reports[0]?.message).toContain('Rename public component directory "composition"');
	});

	it('rejects support files placed directly in a public component directory', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/workbench/Workbench.tsx');
		writeFixture(
			root,
			'packages/example/src/workbench/index.ts',
			"export { Workbench } from './Workbench';\n",
		);
		writeFixture(root, 'packages/example/src/workbench/use-workbench.ts');
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Workbench', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('Move direct file "use-workbench.ts"');
	});

	it('rejects a private child component left loose in its source module', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/WorkbenchToolbar.tsx');
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
			'packages/example/src/composition/components/toolbar/components/action/Action.tsx',
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

	it('rejects direct support files and private barrels in a private component group', () => {
		const root = createFixture();
		const filename = writeFixture(
			root,
			'packages/example/src/workbench/components/toolbar/Toolbar.tsx',
		);
		writeFixture(root, 'packages/example/src/workbench/components/toolbar/index.ts');
		writeFixture(root, 'packages/example/src/workbench/components/toolbar/toolbar-state.ts');
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Toolbar', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports.map((report) => report.message)).toEqual(
			expect.arrayContaining([
				expect.stringContaining('Move direct file "index.ts"'),
				expect.stringContaining('Move direct file "toolbar-state.ts"'),
			]),
		);
	});

	it('rejects a component placed loose directly under components', () => {
		const root = createFixture();
		const filename = writeFixture(root, 'packages/example/src/composition/components/Toolbar.tsx');
		const { context, reports } = createContext(filename);
		const visitors = componentDirectory.create(context);
		const program = { type: 'Program', loc: LOCATION };

		visitors.FunctionDeclaration({
			type: 'FunctionDeclaration',
			parent: program,
			id: { type: 'Identifier', name: 'Toolbar', loc: LOCATION },
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('components/<component-name>/<ComponentName>.tsx');
	});

	it('rejects a CSS Module imported from outside the component group', () => {
		const { context, reports } = createContext(
			'/workspace/packages/example/src/composition/components/toolbar/Toolbar.tsx',
		);

		componentDirectory.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			loc: LOCATION,
			source: { value: '../../toolbar.module.css', loc: LOCATION },
		});

		expect(reports[0]?.message).toContain('must be colocated');
	});

	it('rejects a colocated CSS Module whose name does not match the component', () => {
		const { context, reports } = createContext(
			'/workspace/packages/example/src/workbench/components/toolbar/Toolbar.tsx',
		);

		componentDirectory.create(context).ImportDeclaration({
			type: 'ImportDeclaration',
			loc: LOCATION,
			source: { value: './styles.module.css', loc: LOCATION },
		});

		expect(reports[0]?.message).toContain('"./Toolbar.module.css"');
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

describe('no-react-use-callback', () => {
	it('rejects direct and aliased named imports from React', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/hooks/useWorkbench.ts',
		);
		noReactUseCallback.create(context).ImportDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					imported: { type: 'Identifier', name: 'useCallback' },
					local: { type: 'Identifier', name: 'memoizeHandler' },
					loc: LOCATION,
				},
			],
		});

		expect(reports).toHaveLength(1);
		expect(reports[0]?.message).toContain('useMemoizedFn');
		expect(reports[0]?.message).toContain('docs/frontend/react.md#callback-identity');
	});

	it('rejects namespace, default, computed, and destructured access', () => {
		const { context, reports } = createContext(
			'/workspace/packages/ui/src/components/ui/carousel.tsx',
		);
		const visitors = noReactUseCallback.create(context);
		visitors.ImportDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ImportDefaultSpecifier',
					local: { type: 'Identifier', name: 'React' },
				},
			],
		});
		visitors.MemberExpression({
			type: 'MemberExpression',
			computed: false,
			object: { type: 'Identifier', name: 'React' },
			property: { type: 'Identifier', name: 'useCallback' },
			loc: LOCATION,
		});
		visitors.MemberExpression({
			type: 'MemberExpression',
			computed: true,
			object: { type: 'Identifier', name: 'React' },
			property: { type: 'Literal', value: 'useCallback' },
			loc: LOCATION,
		});
		visitors.VariableDeclarator({
			id: {
				type: 'ObjectPattern',
				properties: [
					{
						type: 'Property',
						key: { type: 'Identifier', name: 'useCallback' },
						loc: LOCATION,
					},
				],
			},
			init: { type: 'Identifier', name: 'React' },
		});

		expect(reports).toHaveLength(3);
	});

	it('rejects named and wildcard re-exports from React', () => {
		const { context, reports } = createContext('/workspace/packages/foundation/src/react/index.ts');
		const visitors = noReactUseCallback.create(context);
		visitors.ExportNamedDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ExportSpecifier',
					local: { type: 'Identifier', name: 'useCallback' },
					exported: { type: 'Identifier', name: 'stableCallback' },
					loc: LOCATION,
				},
			],
		});
		visitors.ExportAllDeclaration({
			source: { value: 'react' },
			loc: LOCATION,
		});

		expect(reports).toHaveLength(2);
	});

	it('accepts useMemoizedFn and unrelated React APIs', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/hooks/useWorkbench.ts',
		);
		const visitors = noReactUseCallback.create(context);
		visitors.ImportDeclaration({
			source: { value: '@orchbee/foundation/react' },
			specifiers: [
				{
					type: 'ImportSpecifier',
					imported: { type: 'Identifier', name: 'useMemoizedFn' },
					local: { type: 'Identifier', name: 'useMemoizedFn' },
					loc: LOCATION,
				},
			],
		});
		visitors.ImportDeclaration({
			source: { value: 'react' },
			specifiers: [
				{
					type: 'ImportNamespaceSpecifier',
					local: { type: 'Identifier', name: 'React' },
				},
			],
		});
		visitors.MemberExpression({
			type: 'MemberExpression',
			computed: false,
			object: { type: 'Identifier', name: 'React' },
			property: { type: 'Identifier', name: 'useMemo' },
			loc: LOCATION,
		});

		expect(reports).toEqual([]);
	});

	it('does not govern tooling outside application and package source trees', () => {
		const { context, reports } = createContext('/workspace/scripts/example.ts');
		const visitors = noReactUseCallback.create(context);

		expect(visitors).toEqual({});
		expect(reports).toEqual([]);
	});
});

describe('one-function-per-tsx', () => {
	it('reports every additional top-level function in authored production TSX', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/Workbench.tsx',
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

	it('accepts a symbol-matched component, its type export, and nested callbacks', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/Workbench.tsx',
		);
		const program = { type: 'Program', loc: LOCATION };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const component = {
			type: 'FunctionDeclaration',
			parent: exportDeclaration,
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
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: component,
			specifiers: [],
		});
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: {
				type: 'TSInterfaceDeclaration',
				id: { type: 'Identifier', name: 'WorkbenchProps', loc: LOCATION },
			},
			specifiers: [],
		});
		visitors['Program:exit'](program);

		expect(reports).toEqual([]);
	});

	it('requires the component filename to match its named runtime export', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/workbench.tsx',
		);
		const program = { type: 'Program', loc: LOCATION };
		const exportDeclaration = { type: 'ExportNamedDeclaration', parent: program };
		const component = {
			type: 'FunctionDeclaration',
			parent: exportDeclaration,
			id: { type: 'Identifier', name: 'Workbench', loc: LOCATION },
		};
		const visitors = oneFunctionPerTsx.create(context);

		visitors.FunctionDeclaration(component);
		visitors.ExportNamedDeclaration({
			...exportDeclaration,
			declaration: component,
			specifiers: [],
		});
		visitors['Program:exit'](program);

		expect(reports[0]?.message).toContain('"Workbench.tsx"');
	});

	it('reserves PascalCase TSX filenames for function components', () => {
		const { context, reports } = createContext(
			'/workspace/apps/web/src/composition/ApplicationEntry.tsx',
		);
		const visitors = oneFunctionPerTsx.create(context);

		visitors['Program:exit']({ type: 'Program', loc: LOCATION });

		expect(reports[0]?.message).toContain('to kebab-case');
	});
});

describe('one-hook-per-file', () => {
	it('accepts one named Hook function exported from its module', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/hooks/useWorkbenchViewProps.ts',
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
			'/workspace/packages/workbench/src/workbench/hooks/useWorkbenchViewProps.ts',
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
			'/workspace/packages/workbench/src/workbench/hooks/useWorkbenchViewProps.ts',
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

	it('requires a Hook filename to match a Hook export discovered from source', () => {
		const { context, reports } = createContext(
			'/workspace/packages/workbench/src/workbench/hooks/workbench-view-props.ts',
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

		expect(reports[0]?.message).toContain('"useWorkbenchViewProps.ts"');
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
