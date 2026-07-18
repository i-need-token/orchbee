import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const workspaceSourceCondition = '@orchbee/source';

function fail(message) {
	throw new Error(message);
}

function requireString(value, field, packageName) {
	if (typeof value !== 'string' || value.length === 0) {
		fail(`${packageName}: package.json#${field} must be a non-empty string`);
	}
	return value;
}

function requireObject(value, field, packageName) {
	if (value === null || typeof value !== 'object' || Array.isArray(value)) {
		fail(`${packageName}: package.json#${field} must be an object`);
	}
	return value;
}

function exportTargetPath(packageRoot, target, field, packageName) {
	if (!target.startsWith('./')) {
		fail(`${packageName}: package.json#${field} must use a package-relative target`);
	}
	const targetPath = resolve(packageRoot, target);
	const relativeTarget = relative(packageRoot, targetPath);
	if (relativeTarget.startsWith(`..${sep}`) || isAbsolute(relativeTarget)) {
		fail(`${packageName}: package.json#${field} escapes the package root`);
	}
	return targetPath;
}

async function packageFiles(packageRoot) {
	const { stdout } = await execFileAsync('npm', ['pack', '--dry-run', '--json'], {
		cwd: packageRoot,
		maxBuffer: 16 * 1024 * 1024,
	});
	const reports = JSON.parse(stdout);
	const report = reports[0];
	if (report === undefined || !Array.isArray(report.files)) {
		fail(`${packageRoot}: npm pack did not return a file manifest`);
	}
	return new Set(report.files.map((file) => file.path));
}

function exportedTargets(exportsField, packageName) {
	const targets = [];
	for (const [subpath, exportValue] of Object.entries(exportsField)) {
		if (typeof exportValue === 'string') {
			targets.push([`exports[${JSON.stringify(subpath)}]`, exportValue]);
			continue;
		}
		const conditions = requireObject(
			exportValue,
			`exports[${JSON.stringify(subpath)}]`,
			packageName,
		);
		if (
			workspaceSourceCondition in conditions &&
			Object.keys(conditions)[0] !== workspaceSourceCondition
		) {
			fail(
				`${packageName}: ${workspaceSourceCondition} must be the first condition in exports[${JSON.stringify(subpath)}]`,
			);
		}
		for (const [condition, target] of Object.entries(conditions)) {
			targets.push([
				`exports[${JSON.stringify(subpath)}].${condition}`,
				requireString(target, `exports[${JSON.stringify(subpath)}].${condition}`, packageName),
				condition,
			]);
		}
	}
	return targets;
}

async function verifyPackedTargets(packageRoot, manifest, packageName, exportsField) {
	const files = await packageFiles(packageRoot);
	for (const [field, target, condition] of exportedTargets(exportsField, packageName)) {
		const targetPath = exportTargetPath(packageRoot, target, field, packageName);
		if (condition === workspaceSourceCondition) {
			if (!existsSync(targetPath))
				fail(`${packageName}: ${field} target "${target}" does not exist`);
			if (manifest.private === true) continue;
		}
		const packedPath = relative(packageRoot, targetPath).split(sep).join('/');
		if (!files.has(packedPath)) {
			fail(`${packageName}: ${field} target "${target}" is absent from npm pack`);
		}
	}
}

async function verifyRuntimeFormats(packageRoot, manifest, rootExport) {
	const packageName = requireString(manifest.name, 'name', basename(packageRoot));
	const importTarget = requireString(rootExport.import, 'exports["."].import', packageName);
	const requireTarget = requireString(rootExport.require, 'exports["."].require', packageName);
	const importPath = exportTargetPath(
		packageRoot,
		importTarget,
		'exports["."].import',
		packageName,
	);
	const requirePath = exportTargetPath(
		packageRoot,
		requireTarget,
		'exports["."].require',
		packageName,
	);
	const esmApi = await import(pathToFileURL(importPath).href);
	const cjsApi = createRequire(import.meta.url)(requirePath);
	const esmExports = Object.keys(esmApi).toSorted();
	const cjsExports = Object.keys(cjsApi).toSorted();
	if (JSON.stringify(esmExports) !== JSON.stringify(cjsExports)) {
		fail(
			`${packageName}: ESM and CJS exports differ\nESM: ${esmExports.join(', ')}\nCJS: ${cjsExports.join(', ')}`,
		);
	}
}

function findWorkspaceRoot(packageRoot) {
	let candidate = packageRoot;
	while (true) {
		if (existsSync(join(candidate, 'pnpm-workspace.yaml'))) return candidate;
		const parent = dirname(candidate);
		if (parent === candidate) fail(`${packageRoot}: cannot find pnpm-workspace.yaml`);
		candidate = parent;
	}
}

async function verifyDeclarationConsumer(packageRoot, manifest, rootExport) {
	const packageName = requireString(manifest.name, 'name', basename(packageRoot));
	const typesTarget = requireString(rootExport.types, 'exports["."].types', packageName);
	exportTargetPath(packageRoot, typesTarget, 'exports["."].types', packageName);

	const workspaceRoot = findWorkspaceRoot(packageRoot);
	const fixtureRoot = await mkdtemp(join(tmpdir(), 'orchbee-package-consumer-'));
	try {
		const packageLink = join(fixtureRoot, 'node_modules', ...packageName.split('/'));
		await mkdir(dirname(packageLink), { recursive: true });
		await symlink(packageRoot, packageLink, 'dir');
		await writeFile(
			join(fixtureRoot, 'consumer.ts'),
			`import * as packageApi from '${packageName}';\nvoid packageApi;\n`,
		);
		await execFileAsync(
			join(workspaceRoot, 'node_modules', '.bin', 'tsc'),
			[
				'--noEmit',
				'--strict',
				'--skipLibCheck',
				'false',
				'--target',
				'ES2022',
				'--module',
				'NodeNext',
				'--moduleResolution',
				'NodeNext',
				'--lib',
				'ES2022,DOM,DOM.Iterable',
				join(fixtureRoot, 'consumer.ts'),
			],
			{ cwd: fixtureRoot },
		);
	} finally {
		await rm(fixtureRoot, { recursive: true, force: true });
	}
}

async function main() {
	const packageRoot = resolve(process.cwd(), process.argv[2] ?? '.');
	const manifestPath = join(packageRoot, 'package.json');
	const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
	const packageName = requireString(manifest.name, 'name', basename(packageRoot));
	const exportsField = requireObject(manifest.exports, 'exports', packageName);
	const rootExport = requireObject(exportsField['.'], 'exports["."]', packageName);

	await verifyPackedTargets(packageRoot, manifest, packageName, exportsField);
	await verifyRuntimeFormats(packageRoot, manifest, rootExport);
	await verifyDeclarationConsumer(packageRoot, manifest, rootExport);
	process.stdout.write(`${packageName}: package exports verified\n`);
}

await main();
