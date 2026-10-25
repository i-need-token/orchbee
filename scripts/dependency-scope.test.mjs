import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repositoryRoot = fileURLToPath(new URL('../', import.meta.url));
const knipBinary = join(repositoryRoot, 'node_modules/knip/bin/knip.js');
const configuration = JSON.parse(readFileSync(join(repositoryRoot, 'knip.json'), 'utf8'));
const fixtureWorkspaces = [
	'packages/workbench',
	'packages/tab-strip',
	'apps/web',
	'apps/storybook',
];

function write(root, path, source) {
	const target = join(root, path);
	mkdirSync(dirname(target), { recursive: true });
	writeFileSync(target, source);
}

function fixtureScope(workspace) {
	const scope = configuration.workspaces[workspace];
	assert.ok(Array.isArray(scope.entry), `${workspace} must declare its actual entry scope`);
	assert.ok(Array.isArray(scope.project), `${workspace} must declare its actual project scope`);
	if (workspace.startsWith('packages/')) {
		assert.equal(scope.includeEntryExports, true, `${workspace} must check private entry exports`);
	}
	// The fixture has no Java, CSS tools, or host-only dependencies to exempt. Preserve all graph
	// policy and plugin fields, including includeEntryExports, instead of reconstructing the scope.
	return Object.fromEntries(
		Object.entries(scope).filter(
			([key]) => !['ignoreBinaries', 'ignoreDependencies'].includes(key),
		),
	);
}

function writeWorkspace(root, workspace, fields) {
	const name = `@orchbee/${basename(workspace)}`;
	write(
		root,
		`${workspace}/package.json`,
		JSON.stringify({
			name,
			private: true,
			type: 'module',
			scripts: { test: 'node --test src/**/__tests__/*.unit.test.ts' },
			...fields,
		}),
	);
	const link = join(root, 'node_modules', name);
	mkdirSync(dirname(link), { recursive: true });
	symlinkSync(join(root, workspace), link, 'junction');
}

function writeDependency(root, name) {
	write(
		root,
		`node_modules/${name}/package.json`,
		JSON.stringify({ name, version: '1.0.0', main: './index.js' }),
	);
	write(root, `node_modules/${name}/index.js`, 'exports.value = 7;\n');
}

function withFixture(inspect) {
	const temporaryRoot = resolve(tmpdir());
	const root = mkdtempSync(join(temporaryRoot, 'orchbee-dependency-scope-'));
	try {
		assert.equal(configuration.treatConfigHintsAsErrors, true);
		write(
			root,
			'knip.json',
			JSON.stringify({
				...configuration,
				workspaces: Object.fromEntries(
					fixtureWorkspaces.map((workspace) => [workspace, fixtureScope(workspace)]),
				),
			}),
		);
		write(
			root,
			'package.json',
			JSON.stringify({
				name: 'dependency-scope-fixture',
				private: true,
				workspaces: fixtureWorkspaces,
			}),
		);
		writeWorkspace(root, 'packages/workbench', { exports: { '.': './src/index.ts' } });
		writeWorkspace(root, 'packages/tab-strip', {
			exports: { '.': './src/index.ts' },
			dependencies: { '@orchbee/workbench': 'workspace:*' },
		});
		writeWorkspace(root, 'apps/web', {
			dependencies: { '@orchbee/tab-strip': 'workspace:*' },
		});
		const storybookManifest = JSON.parse(
			readFileSync(join(repositoryRoot, 'apps/storybook/package.json'), 'utf8'),
		);
		for (const name of ['@storybook/addon-a11y', 'storybook-react-rsbuild']) {
			assert.ok(Object.hasOwn(storybookManifest.devDependencies, name));
			writeDependency(root, name);
		}
		writeWorkspace(root, 'apps/storybook', {
			devDependencies: { '@storybook/addon-a11y': '1.0.0', 'storybook-react-rsbuild': '1.0.0' },
		});
		write(
			root,
			'packages/workbench/src/index.ts',
			'export const value = 7;\nexport interface RuntimeValue { amount: number }\n',
		);
		write(
			root,
			'packages/tab-strip/src/index.ts',
			"export { useTabStripActions } from './tab-strip/hooks/useTabStripActions';\n",
		);
		write(
			root,
			'packages/tab-strip/src/tab-strip/hooks/useTabStripActions.ts',
			"import { computeValue } from '../../runtime';\nexport function useTabStripActions() { return computeValue(); }\n",
		);
		write(
			root,
			'packages/tab-strip/src/runtime.ts',
			`import { value } from '@orchbee/workbench';
import type { RuntimeValue } from '@orchbee/workbench';
export function computeValue() {
	const snapshot: RuntimeValue = { amount: value };
	return snapshot.amount;
}
`,
		);
		write(root, 'apps/web/src/index.ts', "import './composition/entry';\n");
		write(
			root,
			'apps/web/src/composition/entry.ts',
			"import { useTabStripActions } from '@orchbee/tab-strip';\nprocess.stdout.write(String(useTabStripActions()));\n",
		);
		write(root, 'apps/web/src/runtime.system-fixture.ts', 'export {};\n');
		write(root, 'apps/web/src/__tests__/runtime.unit.test.ts', "import '../composition/entry';\n");
		write(
			root,
			'packages/tab-strip/src/__tests__/runtime.unit.test.ts',
			"import { useTabStripActions } from '../tab-strip/hooks/useTabStripActions';\nvoid useTabStripActions();\n",
		);
		for (const workspace of ['packages/workbench', 'packages/tab-strip', 'apps/web']) {
			write(root, `${workspace}/config.ts`, 'export {};\n');
			write(
				root,
				`${workspace}/stories/fixture.stories.tsx`,
				"import '../config';\nexport default {};\nexport const Default = {};\n",
			);
		}
		write(
			root,
			'apps/storybook/.storybook/main.ts',
			`export default {
	stories: [
		{ directory: '../../../packages/workbench/stories', files: '**/*.stories.@(ts|tsx)' },
		{ directory: '../../../packages/tab-strip/stories', files: '**/*.stories.@(ts|tsx)' },
		{ directory: '../../../apps/web/stories', files: '**/*.stories.@(ts|tsx)' },
	],
	addons: ['@storybook/addon-a11y'],
	framework: 'storybook-react-rsbuild',
};
`,
		);
		write(root, 'apps/storybook/config.ts', 'export {};\n');
		write(root, 'apps/storybook/src/__tests__/host.unit.test.ts', "import '../../config';\n");
		write(
			root,
			'packages/workbench/src/__tests__/pendingActionExecution.ts',
			'export function pendingActionExecution() { return 7; }\n',
		);
		for (const name of ['agent-navigation', 'terminal-navigation']) {
			write(
				root,
				`packages/workbench/src/__tests__/${name}.unit.test.ts`,
				`import { pendingActionExecution } from './pendingActionExecution';
pendingActionExecution();
`,
			);
		}
		inspect(root);
	} finally {
		assert.equal(dirname(root), temporaryRoot);
		assert.ok(basename(root).startsWith('orchbee-dependency-scope-'));
		rmSync(root, { recursive: true, force: true });
	}
}

function analyze(root, production = false) {
	const result = spawnSync(
		process.execPath,
		[knipBinary, '--directory', root, '--reporter', 'json', ...(production ? ['--strict'] : [])],
		{ cwd: root, encoding: 'utf8', timeout: 30_000, windowsHide: true },
	);
	assert.ifError(result.error);
	const output = `${result.stdout}${result.stderr}`;
	assert.equal(result.signal, null, output);
	assert.ok(result.status === 0 || result.status === 1, output);
	const report = JSON.parse(result.stdout);
	assert.ok(Array.isArray(report.issues), output);
	const issues = report.issues.flatMap(({ file, ...categories }) =>
		Object.entries(categories).flatMap(([kind, items]) =>
			items.map(
				({ name }) => `${kind}:${file.replaceAll('\\', '/')}:${name.replaceAll('\\', '/')}`,
			),
		),
	);
	return { status: result.status, output, issues };
}

function assertIssues(result, expected = []) {
	assert.equal(result.status, expected.length === 0 ? 0 : 1, result.output);
	assert.deepEqual(result.issues.toSorted(), expected.toSorted(), result.output);
}

test('bundled subpaths resolve to their declared sources without hiding private exports or missing dependencies', () => {
	withFixture((root) => {
		// Match the real output exclusion and declaration-to-source layout before exercising
		// the JavaScript subpaths from the owning package manifest.
		for (const path of ['.gitignore', 'tsconfig.base.json', 'packages/workbench/tsconfig.json']) {
			write(root, path, readFileSync(join(repositoryRoot, path), 'utf8'));
		}
		const packagePath = 'packages/workbench/package.json';
		const manifest = JSON.parse(readFileSync(join(root, packagePath), 'utf8'));
		const actual = JSON.parse(readFileSync(join(repositoryRoot, packagePath), 'utf8'));
		for (const [subpath, name] of [
			['document', 'documentValue'],
			['semantic-commands', 'semanticValue'],
		]) {
			const targets = actual.exports[`./${subpath}`];
			manifest.exports[`./${subpath}`] = targets;
			for (const [condition, target] of Object.entries(targets)) {
				let contents = `export const ${name} = 11;\n`;
				if (condition === 'require') contents = `exports.${name} = 11;\n`;
				else if (condition === 'types') contents = `export declare const ${name}: number;\n`;
				write(root, `packages/workbench/${target.slice(2)}`, contents);
			}
		}
		write(root, packagePath, JSON.stringify(manifest));
		const consumerPath = 'packages/tab-strip/src/runtime.ts';
		write(
			root,
			consumerPath,
			`${readFileSync(join(root, consumerPath), 'utf8')}
import { documentValue } from '@orchbee/workbench/document';
import { semanticValue } from '@orchbee/workbench/semantic-commands';
void documentValue;
void semanticValue;
`,
		);
		for (const production of [false, true]) assertIssues(analyze(root, production));

		const source = 'packages/workbench/src/workbench/document/index.ts';
		write(
			root,
			source,
			'export const documentValue = 11;\nexport const unusedDocumentValue = 12;\n',
		);
		for (const production of [false, true]) {
			assertIssues(analyze(root, production), [`exports:${source}:unusedDocumentValue`]);
		}

		write(root, source, 'export const documentValue = 11;\n');
		const consumerManifestPath = 'packages/tab-strip/package.json';
		const consumerManifest = JSON.parse(readFileSync(join(root, consumerManifestPath), 'utf8'));
		delete consumerManifest.dependencies['@orchbee/workbench'];
		write(root, consumerManifestPath, JSON.stringify(consumerManifest));
		const rootManifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
		rootManifest.dependencies = { '@orchbee/workbench': 'workspace:*' };
		write(root, 'package.json', JSON.stringify(rootManifest));
		assertIssues(analyze(root));
		assertIssues(analyze(root, true), [
			'dependencies:package.json:@orchbee/workbench',
			`unlisted:${consumerPath}:@orchbee/workbench`,
		]);
	});
});

test('runtime value and type consumers stay reachable while orphan test helpers fail', () => {
	withFixture((root) => {
		for (const production of [false, true]) {
			const valid = analyze(root, production);
			assertIssues(valid);
		}
		write(
			root,
			'packages/workbench/src/__tests__/orphan.ts',
			'export function orphan() { return 9; }\n',
		);
		const complete = analyze(root);
		assertIssues(complete, [
			'files:packages/workbench/src/__tests__/orphan.ts:packages/workbench/src/__tests__/orphan.ts',
		]);
		const production = analyze(root, true);
		assertIssues(production);
	});
});

test('build scripts may use development dependencies that production source cannot borrow', () => {
	withFixture((root) => {
		const path = 'packages/workbench/package.json';
		const manifest = JSON.parse(readFileSync(join(root, path), 'utf8'));
		manifest.devDependencies = { 'fixture-dependency': '1.0.0' };
		manifest.scripts.build = 'node build.mjs';
		write(root, path, JSON.stringify(manifest));
		writeDependency(root, 'fixture-dependency');
		write(
			root,
			'packages/workbench/build.mjs',
			"import { value } from 'fixture-dependency';\nprocess.stdout.write(String(value));\n",
		);
		for (const production of [false, true]) {
			const valid = analyze(root, production);
			assertIssues(valid);
		}
		write(
			root,
			'packages/workbench/src/index.ts',
			"import { value } from 'fixture-dependency';\nexport { value };\nexport interface RuntimeValue { amount: number }\n",
		);
		const production = analyze(root, true);
		assertIssues(production, ['unlisted:packages/workbench/src/index.ts:fixture-dependency']);
	});
});

test('a test consumer cannot keep an unused production module alive in the strict graph', () => {
	withFixture((root) => {
		write(root, 'packages/workbench/src/unused-production.ts', 'export const unused = 9;\n');
		write(
			root,
			'packages/workbench/src/__tests__/production.unit.test.ts',
			"import { unused } from '../unused-production';\nvoid unused;\n",
		);
		const complete = analyze(root);
		assertIssues(complete);
		const production = analyze(root, true);
		assertIssues(production, [
			'files:packages/workbench/src/unused-production.ts:packages/workbench/src/unused-production.ts',
		]);
	});
});

test('strict analysis rejects a runtime dependency borrowed from the workspace root', () => {
	withFixture((root) => {
		const manifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
		manifest.dependencies = { 'fixture-dependency': '1.0.0' };
		write(root, 'package.json', JSON.stringify(manifest));
		writeDependency(root, 'fixture-dependency');
		write(
			root,
			'packages/workbench/src/index.ts',
			"import { value } from 'fixture-dependency';\nexport { value };\nexport interface RuntimeValue { amount: number }\n",
		);
		const complete = analyze(root);
		assertIssues(complete);
		const production = analyze(root, true);
		// Strict lookup cannot credit the child's import to the root declaration, so the root's
		// dependency is also unused; both diagnostics must survive the workspace boundary.
		assertIssues(production, [
			'dependencies:package.json:fixture-dependency',
			'unlisted:packages/workbench/src/index.ts:fixture-dependency',
		]);
	});
});

test('private entry exports require consumers and a test consumer is insufficient in strict mode', () => {
	withFixture((root) => {
		const path = 'packages/workbench/src/index.ts';
		write(root, path, `${readFileSync(join(root, path), 'utf8')}export const unconsumed = 9;\n`);
		const unused = ['exports:packages/workbench/src/index.ts:unconsumed'];
		for (const production of [false, true]) {
			assertIssues(analyze(root, production), unused);
		}
		write(
			root,
			'packages/workbench/src/__tests__/entry.unit.test.ts',
			"import { unconsumed } from '../index';\nvoid unconsumed;\n",
		);
		assertIssues(analyze(root));
		assertIssues(analyze(root, true), unused);
	});
});

test('Storybook discovers development exports without keeping a production component alive', () => {
	withFixture((root) => {
		write(
			root,
			'packages/workbench/src/StoryOnly.tsx',
			'export function StoryOnly() { return null; }\n',
		);
		write(
			root,
			'packages/workbench/stories/fixture.stories.tsx',
			`import '../config';
import { StoryOnly } from '../src/StoryOnly';
export default { component: StoryOnly };
export const Default = { render: StoryOnly };
`,
		);
		assertIssues(analyze(root));
		assertIssues(analyze(root, true), [
			'files:packages/workbench/src/StoryOnly.tsx:packages/workbench/src/StoryOnly.tsx',
		]);
	});
});
