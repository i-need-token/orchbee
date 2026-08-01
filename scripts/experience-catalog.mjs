import { access, readFile, readdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { z } from 'zod';

const REPOSITORY_ROOT = resolve(import.meta.dirname, '..');
const EXPERIENCE_SCHEMA_PATH = join(REPOSITORY_ROOT, 'schemas', 'experience-component.schema.json');
const STORYBOOK_CONFIG_PATH = join(REPOSITORY_ROOT, 'apps', 'storybook', '.storybook', 'main.ts');
const COLLECTIONS = ['apps', 'packages'];
const FILTER_NAMES = new Set(['intent', 'kind', 'owner', 'status']);

const uniqueStrings = z.array(z.string()).superRefine((values, context) => {
	const duplicates = values.filter((value, index) => values.indexOf(value) !== index);
	for (const duplicate of new Set(duplicates)) {
		context.addIssue({
			code: 'custom',
			message: `Duplicate value "${duplicate}"`,
		});
	}
});

const identifier = z.string().regex(/^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$/);
const intent = z.string().regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/);
const repositoryPath = z.string().regex(/^(?:apps|packages)\/[a-z0-9-]+\/.+/);
const storyId = z.string().regex(/^[a-z][a-z0-9-]*--[a-z0-9-]+$/);
const storyReferenceSchema = z.strictObject({
	exportName: z.string().regex(/^[A-Z][A-Za-z0-9]*$/),
	file: repositoryPath,
	id: storyId,
	states: uniqueStrings.pipe(z.array(z.string().min(10)).min(1)),
});

const experienceComponentSchema = z
	.strictObject({
		$schema: z.string().min(1),
		id: identifier,
		kind: z.enum(['experience', 'business']),
		status: z.enum(['candidate', 'reviewed', 'canonical', 'deprecated']),
		name: z.string().min(3),
		description: z.string().min(30),
		owner: z.string().regex(/^@orchbee\/[a-z][a-z0-9-]*$/),
		visibility: z.enum(['private', 'public']),
		intents: uniqueStrings.pipe(z.array(intent).min(1)),
		implementation: uniqueStrings.pipe(z.array(repositoryPath).min(1)),
		stories: z.array(storyReferenceSchema),
		uses: uniqueStrings.pipe(z.array(identifier)),
		useWhen: uniqueStrings.pipe(z.array(z.string().min(15)).min(1)),
		avoidWhen: uniqueStrings.pipe(z.array(z.string().min(15)).min(1)),
		invariants: uniqueStrings.pipe(z.array(z.string().min(15)).min(1)),
		references: uniqueStrings.pipe(z.array(repositoryPath)),
		deprecatedBy: identifier.optional(),
	})
	.superRefine((manifest, context) => {
		if (manifest.status === 'canonical' && manifest.stories.length === 0) {
			context.addIssue({
				code: 'custom',
				message: 'A canonical component must reference at least one Story',
				path: ['stories'],
			});
		}
		if (manifest.status === 'deprecated' && manifest.deprecatedBy === undefined) {
			context.addIssue({
				code: 'custom',
				message: 'A deprecated component must identify its replacement',
				path: ['deprecatedBy'],
			});
		}
		if (manifest.status !== 'deprecated' && manifest.deprecatedBy !== undefined) {
			context.addIssue({
				code: 'custom',
				message: 'Only a deprecated component may identify a replacement',
				path: ['deprecatedBy'],
			});
		}
	});

function fail(message) {
	throw new Error(message);
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
	const entries = await readdir(collectionRoot, { withFileTypes: true });
	const candidates = entries
		.filter((entry) => entry.isDirectory())
		.map((entry) => join(collectionRoot, entry.name));
	const manifests = await Promise.all(
		candidates.map((directory) => fileExists(join(directory, 'package.json'))),
	);
	return candidates.filter((_directory, index) => manifests[index]);
}

async function readJson(path, schema) {
	const value = JSON.parse(await readFile(path, 'utf8'));
	const result = schema.safeParse(value);
	if (!result.success) {
		fail(`${relative(REPOSITORY_ROOT, path)}\n${z.prettifyError(result.error)}`);
	}
	return result.data;
}

async function manifestPaths() {
	const workspaces = (
		await Promise.all(COLLECTIONS.map((collection) => workspaceDirectories(collection)))
	).flat();
	const paths = await Promise.all(
		workspaces.map(async (workspace) => {
			const directory = join(workspace, 'docs', 'experience');
			if (!(await fileExists(directory))) return [];
			const entries = await readdir(directory, { withFileTypes: true });
			return entries
				.filter((entry) => entry.isFile() && entry.name.endsWith('.json'))
				.map((entry) => join(directory, entry.name));
		}),
	);
	return paths.flat().toSorted();
}

function workspaceRootForManifest(path) {
	return resolve(dirname(path), '..', '..');
}

function ensureOwnedPath({ manifestPath, ownerRoot, path, field }) {
	const absolutePath = resolve(REPOSITORY_ROOT, path);
	const ownerRelativePath = relative(ownerRoot, absolutePath);
	if (ownerRelativePath.startsWith('..') || ownerRelativePath === '') {
		fail(
			`${relative(REPOSITORY_ROOT, manifestPath)}: ${field} path "${path}" must stay inside its owner workspace`,
		);
	}
	return absolutePath;
}

function storyIdPart(value) {
	return value
		.replaceAll(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replaceAll(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
		.replaceAll(/[^A-Za-z0-9]+/g, '-')
		.replaceAll(/^-|-$/g, '')
		.toLocaleLowerCase('en-US');
}

async function storyTitlePrefixes() {
	const source = await readFile(STORYBOOK_CONFIG_PATH, 'utf8');
	const pattern = /directory:\s*['"]([^'"]+)['"][\s\S]*?titlePrefix:\s*['"]([^'"]+)['"]/g;
	return [...source.matchAll(pattern)].map((match) => ({
		directory: resolve(dirname(STORYBOOK_CONFIG_PATH), match[1]),
		titlePrefix: match[2],
	}));
}

function storyTitlePrefix({ absolutePath, mappings, storyFile }) {
	const matches = mappings.filter(({ directory }) => {
		const candidate = relative(directory, absolutePath);
		return candidate !== '' && !candidate.startsWith('..');
	});
	if (matches.length !== 1) {
		fail(`${storyFile}: expected exactly one Storybook titlePrefix owner`);
	}
	return matches[0].titlePrefix;
}

async function verifyStoryReference({ manifestPath, ownerRoot, story, titlePrefixes }) {
	const absolutePath = ensureOwnedPath({
		field: 'stories',
		manifestPath,
		ownerRoot,
		path: story.file,
	});
	if (!(await fileExists(absolutePath))) {
		fail(`${relative(REPOSITORY_ROOT, manifestPath)}: stories path "${story.file}" does not exist`);
	}
	const source = await readFile(absolutePath, 'utf8');
	const title = /\btitle:\s*['"]([^'"]+)['"]/.exec(source)?.[1];
	if (title === undefined) {
		fail(`${story.file}: experience Stories require a literal CSF title`);
	}
	const exportPattern = new RegExp(`\\bexport\\s+const\\s+${story.exportName}\\b`);
	if (!exportPattern.test(source)) {
		fail(`${story.file}: missing named Story export "${story.exportName}"`);
	}
	const titlePrefix = storyTitlePrefix({
		absolutePath,
		mappings: titlePrefixes,
		storyFile: story.file,
	});
	const expectedId = `${storyIdPart(`${titlePrefix}/${title}`)}--${storyIdPart(story.exportName)}`;
	if (story.id !== expectedId) {
		fail(
			`${relative(REPOSITORY_ROOT, manifestPath)}: Story ${story.exportName} id must be "${expectedId}"`,
		);
	}
}

function verifyCompositionGraph(manifestsById) {
	const complete = new Set();
	const visit = (id, ancestors) => {
		if (ancestors.includes(id)) {
			fail(`Experience component composition cycle: ${[...ancestors, id].join(' -> ')}`);
		}
		if (complete.has(id)) return;
		const record = manifestsById.get(id);
		if (record === undefined) return;
		for (const dependency of record.manifest.uses) visit(dependency, [...ancestors, id]);
		complete.add(id);
	};
	for (const id of manifestsById.keys()) visit(id, []);
}

async function loadCatalog() {
	const [paths, titlePrefixes] = await Promise.all([manifestPaths(), storyTitlePrefixes()]);
	const records = await Promise.all(
		paths.map(async (path) => {
			const manifest = await readJson(path, experienceComponentSchema);
			const ownerRoot = workspaceRootForManifest(path);
			const packageManifest = await readJson(
				join(ownerRoot, 'package.json'),
				z.looseObject({ name: z.string().min(1) }),
			);
			if (packageManifest.name !== manifest.owner) {
				fail(`${relative(REPOSITORY_ROOT, path)}: owner must be "${packageManifest.name}"`);
			}
			const expectedSchemaPath = relative(dirname(path), EXPERIENCE_SCHEMA_PATH)
				.split('\\')
				.join('/');
			if (manifest.$schema !== expectedSchemaPath) {
				fail(`${relative(REPOSITORY_ROOT, path)}: $schema must be "${expectedSchemaPath}"`);
			}

			const ownedPaths = [
				...manifest.implementation.map((ownedPath) => ({
					field: 'implementation',
					path: ownedPath,
				})),
				...manifest.references.map((reference) => ({
					field: 'references',
					path: reference,
				})),
			];
			await Promise.all(
				ownedPaths.map(async (ownedPath) => {
					const absolutePath = ensureOwnedPath({
						manifestPath: path,
						ownerRoot,
						...ownedPath,
					});
					if (!(await fileExists(absolutePath))) {
						fail(
							`${relative(REPOSITORY_ROOT, path)}: ${ownedPath.field} path "${ownedPath.path}" does not exist`,
						);
					}
				}),
			);
			await Promise.all(
				manifest.stories.map((story) =>
					verifyStoryReference({ manifestPath: path, ownerRoot, story, titlePrefixes }),
				),
			);
			return { manifest, path };
		}),
	);

	const manifestsById = new Map();
	for (const record of records) {
		const existing = manifestsById.get(record.manifest.id);
		if (existing !== undefined) {
			fail(
				`Duplicate experience component id "${record.manifest.id}" in ${relative(REPOSITORY_ROOT, existing.path)} and ${relative(REPOSITORY_ROOT, record.path)}`,
			);
		}
		manifestsById.set(record.manifest.id, record);
	}
	for (const record of records) {
		for (const dependency of record.manifest.uses) {
			if (!manifestsById.has(dependency)) {
				fail(
					`${relative(REPOSITORY_ROOT, record.path)}: uses unknown experience component "${dependency}"`,
				);
			}
			if (dependency === record.manifest.id) {
				fail(`${relative(REPOSITORY_ROOT, record.path)}: a component cannot use itself`);
			}
		}
		if (
			record.manifest.deprecatedBy !== undefined &&
			!manifestsById.has(record.manifest.deprecatedBy)
		) {
			fail(
				`${relative(REPOSITORY_ROOT, record.path)}: deprecatedBy names unknown experience component "${record.manifest.deprecatedBy}"`,
			);
		}
	}
	verifyCompositionGraph(manifestsById);
	return records.toSorted((left, right) => left.manifest.id.localeCompare(right.manifest.id));
}

function parseArguments(arguments_) {
	const filters = {};
	const terms = [];
	for (let index = 0; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		if (!argument?.startsWith('--')) {
			if (argument !== undefined) terms.push(argument);
			continue;
		}
		const name = argument.slice(2);
		if (!FILTER_NAMES.has(name)) fail(`Unknown filter --${name}`);
		const value = arguments_[index + 1];
		if (value === undefined || value.startsWith('--')) fail(`Missing value for --${name}`);
		filters[name] = value;
		index += 1;
	}
	return { filters, terms };
}

function filterCatalog(records, { filters, terms }) {
	const normalizedTerms = terms.map((term) => term.toLocaleLowerCase('en-US'));
	return records.filter(({ manifest }) => {
		if (filters.intent !== undefined && !manifest.intents.includes(filters.intent)) return false;
		if (filters.kind !== undefined && manifest.kind !== filters.kind) return false;
		if (filters.owner !== undefined && manifest.owner !== filters.owner) return false;
		if (filters.status !== undefined && manifest.status !== filters.status) return false;
		const searchable = [manifest.id, manifest.name, manifest.description, ...manifest.intents]
			.join(' ')
			.toLocaleLowerCase('en-US');
		return normalizedTerms.every((term) => searchable.includes(term));
	});
}

function renderSummary(records) {
	if (records.length === 0) return 'No matching experience components.\n';
	return `${records
		.map(
			({ manifest }) =>
				`${manifest.id}\n  ${manifest.kind} · ${manifest.status} · ${manifest.owner} · ${manifest.visibility}\n  ${manifest.description}`,
		)
		.join('\n\n')}\n`;
}

function schemaDocument() {
	return `${JSON.stringify(z.toJSONSchema(experienceComponentSchema), null, '\t')}\n`;
}

function usage() {
	return `Usage:
  pnpm experience:list [--kind <kind>] [--status <status>] [--owner <package>]
  pnpm experience:find [terms...] [--intent <intent>] [--kind <kind>] [--status <status>] [--owner <package>]
  pnpm experience:show <id>
`;
}

async function main() {
	const [command, ...arguments_] = process.argv.slice(2);
	if (command === '--print-schema') {
		process.stdout.write(schemaDocument());
		return;
	}

	const actualSchema = JSON.parse(await readFile(EXPERIENCE_SCHEMA_PATH, 'utf8'));
	const expectedSchema = z.toJSONSchema(experienceComponentSchema);
	if (JSON.stringify(actualSchema) !== JSON.stringify(expectedSchema)) {
		fail('schemas/experience-component.schema.json is stale; regenerate it from the Zod contract');
	}

	const records = await loadCatalog();
	if (command === 'check') {
		process.stdout.write(`Verified ${records.length} experience component manifest(s)\n`);
		return;
	}
	if (command === 'list') {
		process.stdout.write(renderSummary(filterCatalog(records, parseArguments(arguments_))));
		return;
	}
	if (command === 'find') {
		process.stdout.write(renderSummary(filterCatalog(records, parseArguments(arguments_))));
		return;
	}
	if (command === 'show') {
		const [id, ...unexpected] = arguments_;
		if (id === undefined || unexpected.length > 0) fail(usage());
		const record = records.find(({ manifest }) => manifest.id === id);
		if (record === undefined) fail(`Unknown experience component "${id}"`);
		process.stdout.write(`${JSON.stringify(record.manifest, null, '\t')}\n`);
		return;
	}
	if (command === undefined) fail(usage());
	fail(`Unknown command "${command}"\n${usage()}`);
}

const entryPath = process.argv[1];
if (entryPath !== undefined && import.meta.url === pathToFileURL(resolve(entryPath)).href) {
	await main();
}

export { experienceComponentSchema, filterCatalog, loadCatalog, renderSummary };
