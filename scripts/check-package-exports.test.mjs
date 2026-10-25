import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repositoryRoot = fileURLToPath(new URL('../', import.meta.url));
const checker = join(repositoryRoot, 'scripts/check-package-exports.mjs');

function write(root, path, source) {
	const target = join(root, path);
	mkdirSync(dirname(target), { recursive: true });
	writeFileSync(target, source);
}

function withPackage(inspect) {
	const temporaryRoot = resolve(tmpdir());
	const root = mkdtempSync(join(temporaryRoot, 'orchbee-export-contract-'));
	const packageRoot = join(root, 'packages/example');
	const manifest = {
		name: '@orchbee/export-fixture',
		version: '1.0.0',
		private: true,
		type: 'module',
		files: ['dist'],
		exports: {
			'./capability': {
				'@orchbee/source': './src/capability.ts',
				types: './dist/capability.d.ts',
				import: './dist/capability.js',
				require: './dist/capability.cjs',
			},
			'./tokens.css': { '@orchbee/source': './src/tokens.css', default: './dist/tokens.css' },
		},
	};
	try {
		write(root, 'pnpm-workspace.yaml', "packages:\n  - 'packages/*'\n");
		write(packageRoot, 'src/capability.ts', 'export const value = 1;\n');
		write(packageRoot, 'dist/capability.js', 'export const value = 1;\n');
		write(packageRoot, 'dist/capability.cjs', 'exports.value = 1;\n');
		write(packageRoot, 'dist/capability.d.ts', 'export declare const value: number;\n');
		for (const directory of ['src', 'dist'])
			write(packageRoot, `${directory}/tokens.css`, ':root { --value: 1; }\n');
		mkdirSync(join(root, 'node_modules'));
		symlinkSync(
			join(repositoryRoot, 'node_modules/typescript'),
			join(root, 'node_modules/typescript'),
			'junction',
		);
		inspect(packageRoot, manifest);
	} finally {
		assert.equal(dirname(root), temporaryRoot);
		assert.ok(basename(root).startsWith('orchbee-export-contract-'));
		rmSync(root, { recursive: true, force: true });
	}
}

function check(packageRoot, manifest, failure) {
	write(packageRoot, 'package.json', JSON.stringify(manifest));
	const result = spawnSync(process.execPath, [checker, packageRoot], {
		encoding: 'utf8',
		timeout: 30_000,
		windowsHide: true,
	});
	assert.ifError(result.error);
	const output = `${result.stdout}${result.stderr}`;
	assert.equal(result.signal, null, output);
	assert.equal(result.status, failure === undefined ? 0 : 1, output);
	if (failure !== undefined) assert.match(output, failure);
}

for (const hasRoot of [false, true]) {
	test(`code subpaths retain runtime and declaration checks ${hasRoot ? 'with' : 'without'} a root`, () => {
		withPackage((root, manifest) => {
			if (hasRoot) {
				manifest.exports['.'] = {
					types: './dist/index.d.ts',
					import: './dist/index.js',
					require: './dist/index.cjs',
				};
				write(root, 'dist/index.js', 'export const rootValue = 2;\n');
				write(root, 'dist/index.cjs', 'exports.rootValue = 2;\n');
				write(root, 'dist/index.d.ts', 'export declare const rootValue: number;\n');
			}
			check(root, manifest);
			write(root, 'dist/capability.cjs', 'exports.different = 1;\n');
			check(root, manifest, /ESM and CJS exports differ/u);
			write(root, 'dist/capability.cjs', 'exports.value = 1;\n');
			write(root, 'dist/capability.d.ts', 'export declare const value: MissingDeclaration;\n');
			check(root, manifest, /MissingDeclaration/u);
		});
	});
}

test('asset-only packages still require packed assets and real source targets', () => {
	withPackage((root, manifest) => {
		delete manifest.exports['./capability'];
		check(root, manifest);
		manifest.files = ['dist/*.js'];
		check(root, manifest, /tokens\.css.*absent from npm pack/u);
		manifest.files = ['dist'];
		manifest.exports['./tokens.css']['@orchbee/source'] = './src/missing.css';
		check(root, manifest, /missing\.css.*does not exist/u);
	});
});

test('only private packages with real exports may omit their root', () => {
	withPackage((root, manifest) => {
		manifest.private = false;
		check(root, manifest, /publishable package must export its root/u);
		manifest.private = true;
		for (const field of ['main', 'module', 'types']) {
			manifest[field] = './dist/capability.js';
			check(root, manifest, /when the private package has no root export/u);
			delete manifest[field];
		}
		manifest.exports = {};
		check(root, manifest, /exports must declare a real entry or asset/u);
	});
});
