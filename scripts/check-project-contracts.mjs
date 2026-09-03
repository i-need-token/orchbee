import { access, readFile, readdir } from 'node:fs/promises';
import { basename, join, relative, resolve } from 'node:path';
import { z } from 'zod';

const REPOSITORY_ROOT = resolve(import.meta.dirname, '..');
const PROJECT_SCHEMA_PATH = join(REPOSITORY_ROOT, 'schemas', 'project-contract.schema.json');
const REQUIRED_LIBRARY_SCRIPTS = [
	'build',
	'dev',
	'typecheck',
	'test',
	'test:watch',
	'test:exports',
];

const uniqueStrings = z.array(z.string()).superRefine((values, context) => {
	const duplicates = values.filter((value, index) => values.indexOf(value) !== index);
	for (const duplicate of new Set(duplicates)) {
		context.addIssue({
			code: 'custom',
			message: `Duplicate value "${duplicate}"`,
		});
	}
});

const projectContractSchema = z.strictObject({
	$schema: z.string().min(1),
	domain: z.string().regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/),
	runtime: uniqueStrings.pipe(
		z.array(z.enum(['browser', 'electron', 'native', 'node', 'universal'])).min(1),
	),
	stability: z.enum(['experimental', 'stable', 'deprecated']),
	responsibility: z.string().min(20),
	nonGoals: uniqueStrings.pipe(z.array(z.string().min(10)).min(1)),
});

const packageManifestSchema = z.looseObject({
	name: z.string().min(1),
	scripts: z.record(z.string(), z.string()).default({}),
});

function fail(message) {
	throw new Error(message);
}

async function readJson(path, schema) {
	const value = JSON.parse(await readFile(path, 'utf8'));
	const result = schema.safeParse(value);
	if (!result.success) fail(`${relative(REPOSITORY_ROOT, path)}\n${z.prettifyError(result.error)}`);
	return result.data;
}

async function fileExists(path) {
	try {
		await access(path);
		return true;
	} catch {
		return false;
	}
}

async function workspaceDirectories(collection) {
	const collectionRoot = join(REPOSITORY_ROOT, collection);
	if (!(await fileExists(collectionRoot))) return [];
	const entries = await readdir(collectionRoot, { withFileTypes: true });
	const candidates = entries
		.filter((entry) => entry.isDirectory())
		.map((entry) => join(collectionRoot, entry.name));
	const manifests = await Promise.all(
		candidates.map(async (directory) => {
			const [hasPackageManifest, hasCargoManifest] = await Promise.all([
				fileExists(join(directory, 'package.json')),
				fileExists(join(directory, 'Cargo.toml')),
			]);
			return hasPackageManifest || hasCargoManifest;
		}),
	);
	return candidates.filter((_directory, index) => manifests[index]);
}

function verifyLibraryManifest(projectRoot, manifest, contractPath) {
	const expectedName = `@orchbee/${basename(projectRoot)}`;
	if (manifest.name !== expectedName) {
		fail(
			`${relative(REPOSITORY_ROOT, join(projectRoot, 'package.json'))}: package name must be ${expectedName}`,
		);
	}
	const missingScripts = REQUIRED_LIBRARY_SCRIPTS.filter(
		(script) => manifest.scripts[script] === undefined,
	);
	if (missingScripts.length > 0) {
		fail(
			`${relative(REPOSITORY_ROOT, contractPath)}: package.json is missing required scripts: ${missingScripts.join(', ')}`,
		);
	}
}

async function verifyProject(projectRoot, collection) {
	const contractPath = join(projectRoot, 'project.json');
	const packageManifestPath = join(projectRoot, 'package.json');
	const cargoManifestPath = join(projectRoot, 'Cargo.toml');
	const [contract, hasPackageManifest, hasCargoManifest] = await Promise.all([
		readJson(contractPath, projectContractSchema),
		fileExists(packageManifestPath),
		fileExists(cargoManifestPath),
	]);
	const expectedSchemaPath = relative(projectRoot, PROJECT_SCHEMA_PATH).split('\\').join('/');
	if (contract.$schema !== expectedSchemaPath) {
		fail(`${relative(REPOSITORY_ROOT, contractPath)}: $schema must be "${expectedSchemaPath}"`);
	}
	if (collection === 'packages') {
		if (!hasPackageManifest)
			fail(`${relative(REPOSITORY_ROOT, projectRoot)} requires package.json`);
		const manifest = await readJson(packageManifestPath, packageManifestSchema);
		verifyLibraryManifest(projectRoot, manifest, contractPath);
	}
	if (collection === 'crates' && !hasCargoManifest) {
		fail(`${relative(REPOSITORY_ROOT, projectRoot)} requires Cargo.toml`);
	}
}

function schemaDocument() {
	return `${JSON.stringify(z.toJSONSchema(projectContractSchema), null, '\t')}\n`;
}

async function main() {
	if (process.argv.includes('--print-schema')) {
		process.stdout.write(schemaDocument());
		return;
	}

	const actualSchema = JSON.parse(await readFile(PROJECT_SCHEMA_PATH, 'utf8'));
	const expectedSchema = z.toJSONSchema(projectContractSchema);
	if (JSON.stringify(actualSchema) !== JSON.stringify(expectedSchema)) {
		fail('schemas/project-contract.schema.json is stale; regenerate it from the Zod contract');
	}

	const collections = ['apps', 'crates', 'packages'];
	const projects = await Promise.all(
		collections.map(async (collection) => ({
			collection,
			directories: await workspaceDirectories(collection),
		})),
	);
	await Promise.all(
		projects.flatMap(({ collection, directories }) =>
			directories.map((directory) => verifyProject(directory, collection)),
		),
	);
	const projectCount = projects.reduce((count, project) => count + project.directories.length, 0);
	process.stdout.write(`Verified ${projectCount} project contract(s)\n`);
}

await main();
