import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { access, lstat, readdir, readlink, realpath } from 'node:fs/promises';
import path from 'node:path';

import { sha256Bytes } from './model-inputs.mjs';
import { inventoryTestPaths } from './model-inventory.mjs';
import { inside, rejectLinks } from './model-records.mjs';

async function fileDigest(file) {
	const digest = createHash('sha256');
	for await (const bytes of createReadStream(file)) digest.update(bytes);
	return digest.digest('hex');
}

async function resolveExecutable(command) {
	const directories =
		command.includes('/') || command.includes('\\')
			? ['']
			: (process.env.PATH ?? '').split(path.delimiter);
	const suffixes = process.platform === 'win32' && !path.extname(command) ? ['.exe'] : [''];
	for (const directory of directories)
		for (const suffix of suffixes) {
			const candidate = path.resolve(directory, `${command}${suffix}`);
			try {
				await access(candidate);
				return await realpath(candidate);
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
		}
	throw new Error(`Cannot resolve concrete Java executable: ${command}`);
}

async function treeDigest(root) {
	const digest = createHash('sha256');
	async function visit(directory, ancestors = new Set()) {
		const canonical = await realpath(directory);
		if (!inside(root, canonical) || ancestors.has(canonical))
			throw new Error('Java runtime link escapes or cycles');
		const next = new Set([...ancestors, canonical]);
		for (const entry of (await readdir(directory, { withFileTypes: true })).sort((left, right) =>
			left.name.localeCompare(right.name, 'en'),
		)) {
			const file = path.join(directory, entry.name);
			let kind = entry;
			if (entry.isSymbolicLink()) {
				const target = await realpath(file);
				if (!inside(root, target)) throw new Error(`Java runtime link escapes: ${file}`);
				digest
					.update(path.relative(root, file))
					.update('\0')
					.update(await readlink(file))
					.update('\0');
				kind = await lstat(target);
			}
			if (kind.isDirectory()) await visit(file, next);
			else if (kind.isFile())
				digest
					.update(path.relative(root, file))
					.update('\0')
					.update(await fileDigest(file))
					.update('\0');
			else throw new Error(`Unsupported Java runtime input: ${file}`);
		}
	}
	await visit(root);
	return digest.digest('hex');
}

export async function cacheRuntimeIdentity(repository, javaCommand, jar, resources) {
	for (const name of [
		'NODE_OPTIONS',
		'JAVA_TOOL_OPTIONS',
		'_JAVA_OPTIONS',
		'JDK_JAVA_OPTIONS',
		'CLASSPATH',
		'TLA_LIBRARY',
	])
		if (process.env[name])
			throw new Error(
				`TLC cache does not support injected ${name}; run explicitly without --cache-dir`,
			);
	if (process.execArgv.length)
		throw new Error('TLC cache does not support injected Node arguments');
	const javaExecutable = await resolveExecutable(javaCommand);
	if (!(await lstat(javaExecutable)).isFile())
		throw new Error('Java executable must be a regular file');
	const probe = spawnSync(javaExecutable, ['-XshowSettings:properties', '-version'], {
		cwd: repository,
		encoding: 'utf8',
	});
	if (probe.error || probe.status !== 0 || probe.signal)
		throw new Error('Java identity probe failed', { cause: probe.error });
	const javaIdentity = probe.stdout + probe.stderr;
	const home = javaIdentity.match(/^\s*java\.home = (.+)$/mu)?.[1]?.trim();
	if (!home) throw new Error('Java did not report its runtime home');
	await rejectLinks(path.resolve(home));
	const javaHome = await realpath(home);
	const expected = await realpath(
		path.join(javaHome, 'bin', process.platform === 'win32' ? 'java.exe' : 'java'),
	);
	if (javaExecutable !== expected)
		throw new Error('Java launcher does not belong to its reported runtime');
	const helperEntries = [];
	async function helperInputs(relative = '') {
		const directory = path.join(repository, 'scripts/tla', relative);
		for (const entry of (await readdir(directory, { withFileTypes: true })).sort((left, right) =>
			left.name.localeCompare(right.name, 'en'),
		)) {
			const name = relative ? `${relative}/${entry.name}` : entry.name;
			const file = path.join(directory, entry.name);
			await rejectLinks(file);
			if (entry.isDirectory()) await helperInputs(name);
			else if (entry.isFile()) {
				if (
					name === 'models.json' ||
					name === 'README.md' ||
					name === 'model-cache-cli.test.mjs' ||
					inventoryTestPaths.has(`scripts/tla/${name}`)
				)
					continue;
				helperEntries.push([name, await fileDigest(file)]);
			} else throw new Error(`Unsupported TLC tooling input: ${file}`);
		}
	}
	await helperInputs();
	return {
		javaExecutable,
		javaIdentity,
		javaSha256: await fileDigest(javaExecutable),
		javaRuntimeSha256: await treeDigest(javaHome),
		jarSha256: await fileDigest(jar),
		helpers: Object.fromEntries(helperEntries),
		nodeExecutable: process.execPath,
		nodeSha256: await fileDigest(process.execPath),
		nodeVersion: process.version,
		platform: process.platform,
		architecture: process.arch,
		environmentSha256: sha256Bytes(
			JSON.stringify(
				Object.entries(process.env).toSorted(([left], [right]) => left.localeCompare(right, 'en')),
			),
		),
		arguments: ['-XX:+UseParallelGC', '-fp', '0', '-lncheck', 'final', '-coverage', '1'],
		resources,
	};
}
