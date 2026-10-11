import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repositoryRoot = fileURLToPath(new URL('../', import.meta.url));
const knipBinary = join(repositoryRoot, 'node_modules/knip/bin/knip.js');
const configuration = JSON.parse(readFileSync(join(repositoryRoot, 'knip.json'), 'utf8'));

function write(root, path, source) {
	const target = join(root, path);
	mkdirSync(dirname(target), { recursive: true });
	writeFileSync(target, source);
}

function withFixture(inspect) {
	const temporaryRoot = resolve(tmpdir());
	const root = mkdtempSync(join(temporaryRoot, 'orchbee-dependency-scope-'));
	try {
		const workbench = configuration.workspaces['packages/workbench'];
		write(
			root,
			'knip.json',
			JSON.stringify({
				treatConfigHintsAsErrors: true,
				workspaces: {
					'packages/workbench': { entry: workbench.entry, project: workbench.project },
				},
			}),
		);
		write(
			root,
			'package.json',
			JSON.stringify({
				name: 'dependency-scope-fixture',
				private: true,
				workspaces: ['packages/workbench'],
			}),
		);
		write(
			root,
			'packages/workbench/package.json',
			JSON.stringify({
				name: '@orchbee/workbench',
				private: true,
				type: 'module',
				exports: { '.': './src/index.ts' },
				scripts: { test: 'node --test src/**/__tests__/*.unit.test.ts' },
			}),
		);
		write(root, 'packages/workbench/src/index.ts', 'export const value = 7;\n');
		write(
			root,
			'packages/workbench/stories/fixture.stories.tsx',
			"import '../config';\nexport const story = {};\n",
		);
		write(root, 'packages/workbench/config.ts', 'export {};\n');
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
		[knipBinary, '--directory', root, ...(production ? ['--strict'] : [])],
		{ cwd: root, encoding: 'utf8', timeout: 30_000, windowsHide: true },
	);
	assert.ifError(result.error);
	return { status: result.status, output: `${result.stdout}${result.stderr}` };
}

test('shared test helpers remain reachable and orphan helpers fail the complete graph', () => {
	withFixture((root) => {
		for (const production of [false, true]) {
			const valid = analyze(root, production);
			assert.equal(valid.status, 0, valid.output);
		}
		write(
			root,
			'packages/workbench/src/__tests__/orphan.ts',
			'export function orphan() { return 9; }\n',
		);
		const complete = analyze(root);
		assert.equal(complete.status, 1, complete.output);
		assert.match(complete.output, /Unused files/u);
		assert.match(complete.output, /__tests__[/\\]orphan\.ts/u);
		const production = analyze(root, true);
		assert.equal(production.status, 0, production.output);
	});
});

test('build scripts may use development dependencies that production source cannot borrow', () => {
	withFixture((root) => {
		const path = 'packages/workbench/package.json';
		const manifest = JSON.parse(readFileSync(join(root, path), 'utf8'));
		manifest.devDependencies = { 'fixture-dependency': '1.0.0' };
		manifest.scripts.build = 'node build.mjs';
		write(root, path, JSON.stringify(manifest));
		write(
			root,
			'node_modules/fixture-dependency/package.json',
			JSON.stringify({ name: 'fixture-dependency', version: '1.0.0', main: './index.js' }),
		);
		write(root, 'node_modules/fixture-dependency/index.js', 'exports.value = 7;\n');
		write(
			root,
			'packages/workbench/build.mjs',
			"import { value } from 'fixture-dependency';\nprocess.stdout.write(String(value));\n",
		);
		for (const production of [false, true]) {
			const valid = analyze(root, production);
			assert.equal(valid.status, 0, valid.output);
		}
		write(
			root,
			'packages/workbench/src/index.ts',
			"import { value } from 'fixture-dependency';\nexport { value };\n",
		);
		const production = analyze(root, true);
		assert.equal(production.status, 1, production.output);
		assert.match(production.output, /Unlisted dependencies/u);
		assert.match(production.output, /fixture-dependency/u);
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
		assert.equal(complete.status, 0, complete.output);
		const production = analyze(root, true);
		assert.equal(production.status, 1, production.output);
		assert.match(production.output, /Unused files/u);
		assert.match(production.output, /unused-production\.ts/u);
	});
});

test('strict analysis rejects a runtime dependency borrowed from the workspace root', () => {
	withFixture((root) => {
		const manifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
		manifest.dependencies = { 'fixture-dependency': '1.0.0' };
		write(root, 'package.json', JSON.stringify(manifest));
		write(
			root,
			'node_modules/fixture-dependency/package.json',
			JSON.stringify({ name: 'fixture-dependency', version: '1.0.0', main: './index.js' }),
		);
		write(root, 'node_modules/fixture-dependency/index.js', 'exports.value = 7;\n');
		write(
			root,
			'packages/workbench/src/index.ts',
			"import { value } from 'fixture-dependency';\nexport { value };\n",
		);
		const complete = analyze(root);
		assert.equal(complete.status, 0, complete.output);
		const production = analyze(root, true);
		assert.equal(production.status, 1, production.output);
		assert.match(production.output, /Unlisted dependencies/u);
		assert.match(production.output, /fixture-dependency/u);
	});
});
