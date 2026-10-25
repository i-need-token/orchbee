import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { assessDependencyReports, readRetentionPolicy } from './dependency-retentions.mjs';

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

function withFixture(inspect, additionalWorkspaces = []) {
	const workspaces = [...fixtureWorkspaces, ...additionalWorkspaces];
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
					workspaces.map((workspace) => [workspace, fixtureScope(workspace)]),
				),
			}),
		);
		write(
			root,
			'package.json',
			JSON.stringify({
				name: 'dependency-scope-fixture',
				private: true,
				workspaces,
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

test('Agent private compiled references resolve to source with and without output and preserve negative witnesses', () => {
	withFixture(
		(root) => {
			const workspace = 'packages/agent';
			const actual = JSON.parse(
				readFileSync(join(repositoryRoot, workspace, 'package.json'), 'utf8'),
			);
			for (const path of [
				'.gitignore',
				'tsconfig.base.json',
				`${workspace}/tsconfig.json`,
				`${workspace}/tsconfig.build.json`,
				`${workspace}/rslib.config.ts`,
				`${workspace}/vitest.config.ts`,
				`${workspace}/conformance/build-reference.mjs`,
			]) {
				write(root, path, readFileSync(join(repositoryRoot, path), 'utf8'));
			}
			writeWorkspace(root, workspace, {
				exports: actual.exports,
				devDependencies: {
					'@rslib/core': actual.devDependencies['@rslib/core'],
					vitest: actual.devDependencies.vitest,
				},
				scripts: {
					build: actual.scripts.build,
					test: actual.scripts.test,
					'conformance:build': actual.scripts['conformance:build'],
					'conformance:generate': actual.scripts['conformance:generate'],
				},
			});
			for (const dependency of ['@rslib/core', 'vitest']) {
				const dependencyLink = join(root, 'node_modules', dependency);
				mkdirSync(dirname(dependencyLink), { recursive: true });
				symlinkSync(
					join(repositoryRoot, workspace, 'node_modules', dependency),
					dependencyLink,
					'junction',
				);
			}
			const source = `${workspace}/src/__tests__/conformance/agent-reference.ts`;
			const contracts = `${workspace}/src/__tests__/conformance/agent-reference-contracts.ts`;
			const generator = `${workspace}/conformance/generate-corpus.mjs`;
			const suite = `${workspace}/src/__tests__/agent-reference.unit.test.ts`;
			const helper =
				"import type { ReferenceValue } from './agent-reference-contracts';\nexport function projectAgentSessionSummary(value: ReferenceValue): number { return value.amount; }\n";
			const consumer =
				"import { projectAgentSessionSummary } from '../dist/__tests__/conformance/agent-reference.js';\nvoid projectAgentSessionSummary({ amount: 7 });\n";
			write(root, source, helper);
			write(root, contracts, 'export interface ReferenceValue { amount: number }\n');
			write(root, generator, consumer);
			write(root, `${workspace}/src/index.ts`, "export { AgentRuntime } from './AgentRuntime';\n");
			write(root, `${workspace}/src/AgentRuntime.ts`, 'export class AgentRuntime {}\n');
			write(
				root,
				`${workspace}/src/__tests__/runtime.unit.test.ts`,
				"import { AgentRuntime } from '../AgentRuntime';\nvoid new AgentRuntime();\n",
			);
			// The production build precedes the private build. Keep its real manifest targets
			// present so the missing-output witness isolates only the private reference bundle.
			for (const [condition, contents] of [
				['types', 'export declare class AgentRuntime {}\n'],
				['import', 'export class AgentRuntime {}\n'],
				['require', 'exports.AgentRuntime = class AgentRuntime {};\n'],
			]) {
				write(root, `${workspace}/${actual.exports['.'][condition].slice(2)}`, contents);
			}
			const webManifest = JSON.parse(readFileSync(join(root, 'apps/web/package.json'), 'utf8'));
			webManifest.dependencies['@orchbee/agent'] = 'workspace:*';
			write(root, 'apps/web/package.json', JSON.stringify(webManifest));
			const webEntry = 'apps/web/src/composition/entry.ts';
			const originalEntry = readFileSync(join(root, webEntry), 'utf8');
			write(
				root,
				webEntry,
				`${originalEntry}\nimport { AgentRuntime } from '@orchbee/agent';\nvoid new AgentRuntime();\n`,
			);

			// No suite imports the reference here: only the real generator import can discover it.
			for (const production of [false, true]) assertIssues(analyze(root, production));
			write(
				root,
				`${workspace}/dist/__tests__/conformance/agent-reference.js`,
				'export function projectAgentSessionSummary(value) { return value.amount; }\n',
			);
			for (const production of [false, true]) assertIssues(analyze(root, production));

			write(root, source, `${helper}export function unusedReference() { return 9; }\n`);
			assertIssues(analyze(root), [`exports:${source}:unusedReference`]);
			assertIssues(analyze(root, true));
			write(root, source, helper);
			write(
				root,
				suite,
				"import { projectAgentSessionSummary } from './conformance/agent-reference';\nvoid projectAgentSessionSummary({ amount: 7 });\n",
			);
			write(root, generator, 'export {};\n');
			assertIssues(analyze(root));
			write(root, suite, 'export {};\n');
			assertIssues(analyze(root), [`files:${source}:${source}`, `files:${contracts}:${contracts}`]);
			assertIssues(analyze(root, true));
			write(root, generator, consumer);

			const index = `${workspace}/src/index.ts`;
			const runtimeExport = readFileSync(join(root, index), 'utf8');
			write(root, index, `${runtimeExport}export const testOnly = 9;\n`);
			write(
				root,
				`${workspace}/src/__tests__/entry.unit.test.ts`,
				"import { testOnly } from '../index';\nvoid testOnly;\n",
			);
			assertIssues(analyze(root));
			assertIssues(analyze(root, true), [`exports:${index}:testOnly`]);
			write(root, index, runtimeExport);
			write(root, `${workspace}/src/__tests__/entry.unit.test.ts`, 'export {};\n');

			write(root, webEntry, `${originalEntry}\nimport '@orchbee/agent';\n`);
			assertIssues(analyze(root, true), [
				`exports:${workspace}/src/AgentRuntime.ts:AgentRuntime`,
				`exports:${index}:AgentRuntime`,
			]);
		},
		['packages/agent'],
	);
});

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

for (const [workspace, helper] of [
	['packages/workbench-client-protocol', 'src/__tests__/conformance/bindings.ts'],
	['packages/terminal-protocol', 'src/__tests__/conformance/input-decoders.ts'],
	['packages/tab-strip', 'src/tab-strip/dragging/__tests__/horizontal-group-drag-invariants.ts'],
]) {
	test(`${workspace} conformance scope preserves complete-graph orphans and strict production exports`, () => {
		withFixture(
			(root) => {
				// Clone the actual workspace fields through fixtureScope. Only the package payload is
				// synthetic: the production exclusion must be the one governing the real helper.
				if (!fixtureWorkspaces.includes(workspace)) {
					const name = `@orchbee/${basename(workspace)}`;
					writeWorkspace(root, workspace, { exports: { '.': './src/index.ts' } });
					write(root, `${workspace}/src/index.ts`, 'export const runtime = 7;\n');
					write(root, `${workspace}/config.ts`, 'export {};\n');
					write(
						root,
						`${workspace}/src/__tests__/runtime.unit.test.ts`,
						"import '../../config';\n",
					);
					if (workspace === 'packages/terminal-protocol') {
						write(root, `${workspace}/scripts/fixture.mjs`, 'export {};\n');
					}
					const manifestPath = 'apps/web/package.json';
					const manifest = JSON.parse(readFileSync(join(root, manifestPath), 'utf8'));
					manifest.dependencies[name] = 'workspace:*';
					write(root, manifestPath, JSON.stringify(manifest));
					const entry = 'apps/web/src/composition/entry.ts';
					write(
						root,
						entry,
						`${readFileSync(join(root, entry), 'utf8')}\nimport { runtime } from '${name}';\nvoid runtime;\n`,
					);
				}
				const helperPath = `${workspace}/${helper}`;
				const suitePath = `${workspace}/${dirname(helper).replaceAll('\\', '/')}/conformance.unit.test.ts`;
				const consumer = `import { witness } from './${basename(helper, '.ts')}';\nvoid witness;\n`;
				write(root, helperPath, 'export function witness() { return 7; }\n');
				write(root, suitePath, consumer);
				for (const production of [false, true]) assertIssues(analyze(root, production));

				write(root, suitePath, 'export {};\n');
				assertIssues(analyze(root), [`files:${helperPath}:${helperPath}`]);
				assertIssues(analyze(root, true));

				write(root, suitePath, consumer);
				const source = `${workspace}/src/index.ts`;
				write(
					root,
					source,
					`${readFileSync(join(root, source), 'utf8')}\nexport const testOnly = 9;\n`,
				);
				write(
					root,
					`${workspace}/src/__tests__/entry.unit.test.ts`,
					"import { testOnly } from '../index';\nvoid testOnly;\n",
				);
				assertIssues(analyze(root));
				assertIssues(analyze(root, true), [`exports:${source}:testOnly`]);
			},
			fixtureWorkspaces.includes(workspace) ? [] : [workspace],
		);
	});
}

// These cases cover the newly admitted acceptance boundary; the existing real graph witnesses
// above still reject root/dev/Story/test-only consumers and production imports of test helpers.
function retentionProcessReport(rows, production) {
	const kinds = [
		'files',
		'dependencies',
		'devDependencies',
		'optionalPeerDependencies',
		'unlisted',
		'binaries',
		'unresolved',
		'exports',
		'nsExports',
		'types',
		'nsTypes',
		'enumMembers',
		'namespaceMembers',
		'duplicates',
		'catalog',
		'cycles',
	];
	const report = Object.fromEntries(
		kinds.map((kind) => [
			kind,
			![
				'nsExports',
				'nsTypes',
				'cycles',
				...(production ? ['devDependencies', 'catalog'] : []),
			].includes(kind),
		]),
	);
	const issues = new Map();
	const counters = Object.fromEntries(kinds.map((kind) => [kind, 0]));
	for (const row of rows) {
		if (!issues.has(row.file))
			issues.set(row.file, {
				file: row.file,
				...Object.fromEntries(kinds.filter((kind) => report[kind]).map((kind) => [kind, []])),
			});
		issues.get(row.file)[row.kind].push({ name: row.name });
		counters[row.kind] += 1;
	}
	const evidence = {
		version: 1,
		configFile: 'knip.json',
		production,
		configHintsDisabled: production,
		configHintsAsErrors: true,
		tagHintsDisabled: false,
		tagHintsAsErrors: false,
		report,
		counters: { ...counters, processed: 1, total: 1 },
		configurationHints: [],
		tagHints: [],
	};
	return {
		status: rows.length ? 1 : 0,
		signal: null,
		error: null,
		stdout: `${JSON.stringify({ issues: [...issues.values()] })}\n@@orchbee-knip-evidence@@${JSON.stringify(evidence)}\n`,
		stderr: '',
	};
}

test('retention acceptance rejects new, stale, malformed, dependency and process failures', () => {
	const { expected } = readRetentionPolicy(repositoryRoot);
	const full = retentionProcessReport([], false);
	const strict = retentionProcessReport(expected, true);
	assert.deepEqual(assessDependencyReports(full, strict, expected), {
		accepted: true,
		rawFullExit: 0,
		rawStrictExit: 1,
		retained: expected,
	});
	const unexpected = {
		kind: 'exports',
		file: 'packages/ui/src/components/ui/tabs.tsx',
		name: 'UnadmittedSlot',
	};
	for (const [label, invalid] of [
		['extra diagnostic', retentionProcessReport([...expected, unexpected], true)],
		[
			'same count with a different diagnostic',
			retentionProcessReport([...expected.slice(1), unexpected], true),
		],
		['vanished diagnostic', retentionProcessReport(expected.slice(1), true)],
		['strict zero with stale records', retentionProcessReport([], true)],
		['duplicate diagnostic', retentionProcessReport([...expected, expected[0]], true)],
		[
			'unlisted dependency',
			retentionProcessReport(
				[
					...expected,
					{ kind: 'unlisted', file: 'apps/web/src/index.ts', name: 'borrowed-runtime' },
				],
				true,
			),
		],
		['parse failure', { ...strict, stdout: '{' }],
		[
			'unknown field',
			{ ...strict, stdout: strict.stdout.replace('"issues":', '"future":true,"issues":') },
		],
		['signal', { ...strict, status: null, signal: 'SIGTERM' }],
		['timeout', { ...strict, status: null, error: 'ETIMEDOUT' }],
		['launcher failure', { ...strict, status: null, error: 'ENOENT' }],
		['runtime failure', { ...strict, status: 2 }],
		['unexpected stderr', { ...strict, stderr: 'warning' }],
		[
			'stale configuration',
			{
				...strict,
				stdout: strict.stdout.replace(
					'"configurationHints":[]',
					'"configurationHints":[{"type":"unused"}]',
				),
			},
		],
		[
			'tag hint',
			{ ...strict, stdout: strict.stdout.replace('"tagHints":[]', '"tagHints":["unknown-tag"]') },
		],
	])
		assert.throws(() => assessDependencyReports(full, invalid, expected), undefined, label);
	assert.throws(
		() => assessDependencyReports({ ...full, status: 1 }, strict, expected),
		/Full Knip/u,
	);
	assert.throws(() => assessDependencyReports(full, strict, expected.slice(1)), /Unregistered/u);
});

test('retention fingerprints reject missing or disconnected real consumer and discovery inputs', () => {
	const read = (file) => readFileSync(join(repositoryRoot, file), 'utf8');
	for (const consumer of [
		'packages/ui/stories/tabs.stories.tsx',
		'packages/tab-strip/src/tab-strip/dragging/__tests__/dom-tab-drag-registry.unit.test.ts',
		'packages/tab-strip/src/tab-strip/dragging/__tests__/cross-window-drag-runtime.unit.test.ts',
	]) {
		assert.throws(
			() =>
				readRetentionPolicy(repositoryRoot, (file) => (file === consumer ? undefined : read(file))),
			/Missing input/u,
		);
		// The file and old symbol text still exist, but no render/call/assertion remains connected.
		assert.throws(
			() =>
				readRetentionPolicy(repositoryRoot, (file) =>
					file === consumer ? `/* ${read(file)} */\nexport {};\n` : read(file),
				),
			/Reviewed consumer/u,
		);
	}
	for (const config of [
		'knip.json',
		'apps/storybook/.storybook/main.ts',
		'packages/tab-strip/vitest.config.ts',
	]) {
		assert.throws(
			() => readRetentionPolicy(repositoryRoot, (file) => (file === config ? '{}' : read(file))),
			/configuration changed/u,
		);
	}
});

function retentionEvidenceEnvelope(result) {
	return JSON.parse(
		result.stdout.trim().split('\n').at(-1).slice('@@orchbee-knip-evidence@@'.length),
	);
}

test('retention evidence reporter accepts exact fixture diagnostics and rejects config and tag hints', () => {
	withFixture((root) => {
		const run = (production) => {
			const child = spawnSync(
				process.execPath,
				[
					knipBinary,
					'--directory',
					root,
					...(production ? ['--strict'] : []),
					'--reporter',
					'json',
					'--reporter',
					join(repositoryRoot, 'scripts/dependency-retentions-reporter.config.mjs'),
				],
				{ cwd: root, encoding: 'utf8', timeout: 30_000, windowsHide: true },
			);
			return {
				status: child.status,
				signal: child.signal,
				error: child.error ? String(child.error) : null,
				stdout: child.stdout,
				stderr: child.stderr,
			};
		};
		assert.equal(assessDependencyReports(run(false), run(true), []).accepted, true);
		const retainedFile = 'packages/workbench/src/retained-development.ts';
		write(root, retainedFile, 'export const retained = 9;\n');
		write(
			root,
			'packages/workbench/src/__tests__/retained-development.unit.test.ts',
			"import { retained } from '../retained-development';\nvoid retained;\n",
		);
		const expected = [{ kind: 'files', file: retainedFile, name: retainedFile }];
		const full = run(false);
		const strict = run(true);
		assert.equal(strict.status, 1);
		assert.equal(assessDependencyReports(full, strict, expected).accepted, true);
		const configured = JSON.parse(readFileSync(join(root, 'knip.json'), 'utf8'));
		configured.workspaces['packages/workbench'].entry.push('src/missing-retention-entry.ts');
		write(root, 'knip.json', JSON.stringify(configured));
		const stale = run(false);
		assert.equal(stale.status, 1);
		assert.ok(retentionEvidenceEnvelope(stale).configurationHints.length > 0);
		assert.throws(() => assessDependencyReports(stale, strict, expected), /Full Knip/u);
		configured.workspaces['packages/workbench'].entry.pop();
		configured.workspaces['packages/workbench'].ignoreBinaries = ['retention-fixture-unused'];
		write(root, 'knip.json', JSON.stringify(configured));
		const staleStrict = run(true);
		assert.equal(staleStrict.status, 1);
		assert.equal(retentionEvidenceEnvelope(staleStrict).configHintsDisabled, true);
		assert.ok(retentionEvidenceEnvelope(staleStrict).configurationHints.length > 0);
		assert.throws(
			() => assessDependencyReports(full, staleStrict, expected),
			/configurationHints/u,
		);
		delete configured.workspaces['packages/workbench'].ignoreBinaries;
		write(root, 'knip.json', JSON.stringify(configured));
		write(
			root,
			'packages/workbench/src/index.ts',
			'/** @internal */\nexport const value = 7;\nexport interface RuntimeValue { amount: number }\n',
		);
		const tagged = run(true);
		assert.equal(tagged.status, 1);
		assert.equal(retentionEvidenceEnvelope(tagged).configurationHints.length, 0);
		assert.equal(retentionEvidenceEnvelope(tagged).tagHints.length, 1);
		assert.throws(() => assessDependencyReports(full, tagged, expected), /tagHints/u);
	});
});
