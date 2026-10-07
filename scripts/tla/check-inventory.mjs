import { access, readFile, readdir } from 'node:fs/promises';
import path from 'node:path';

import { checkSteps } from '../run-checks.mjs';
import {
	assertInputAdmission,
	applyConfigConstants,
	inventoryInputPaths,
	inventoryPath,
	inventoryTestPaths,
	repositoryRoot,
	toolingCommand,
	toolingModelId,
	validateInventory,
} from './model-inventory.mjs';
import { validateDocumentScope, validateToolingWorkflow } from './document-scopes.mjs';

const inventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
await assertInputAdmission(inventoryInputPaths(inventory));

async function requireFile(repositoryPath) {
	try {
		return await readFile(path.join(repositoryRoot, repositoryPath), 'utf8');
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			throw new Error(`TLA+ inventory references missing file: ${repositoryPath}`, {
				cause: error,
			});
		}
		throw error;
	}
}

async function fileExists(repositoryPath) {
	try {
		await access(path.join(repositoryRoot, repositoryPath));
		return true;
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			return false;
		}
		throw error;
	}
}

const referencedModules = new Set();
const referencedConfigs = new Set();
const workspaces = new Set();
for (const model of inventory.models) {
	const module = await requireFile(model.module);
	const config = await requireFile(model.config);
	for (const variant of model.variants ?? []) applyConfigConstants(config, variant.constants);
	for (const claim of model.claims) await requireFile(claim);
	for (const scope of model.documentScopes ?? [])
		validateDocumentScope(scope, await requireFile(scope.path));
	const moduleName = path.basename(model.module, '.tla');
	if (!new RegExp(`\\bMODULE\\s+${moduleName}\\b`, 'u').test(module)) {
		throw new Error(`${model.module} does not declare MODULE ${moduleName}`);
	}
	for (const supportingModulePath of model.supportingModules ?? []) {
		const supportingModule = await requireFile(supportingModulePath);
		const supportingModuleName = path.basename(supportingModulePath, '.tla');
		if (!new RegExp(`\\bMODULE\\s+${supportingModuleName}\\b`, 'u').test(supportingModule)) {
			throw new Error(`${supportingModulePath} does not declare MODULE ${supportingModuleName}`);
		}
		referencedModules.add(supportingModulePath);
	}
	if (!/^\s*(?:SPECIFICATION|INIT)\b/mu.test(config)) {
		throw new Error(`${model.config} does not select a TLC specification or INIT/NEXT pair`);
	}
	if (!/^\s*CHECK_DEADLOCK\s+(?:TRUE|FALSE)\s*$/mu.test(config)) {
		throw new Error(`${model.config} must declare CHECK_DEADLOCK explicitly`);
	}
	const definitions = new Set(
		[...module.matchAll(/^([A-Za-z][A-Za-z\d_]*)\s*(?:\([^\n]*\))?\s*==/gmu)].map(
			([, name]) => name,
		),
	);
	for (const marker of model.requiredCoverage) {
		if (!definitions.has(marker)) {
			throw new Error(`${model.module} does not define required coverage operator ${marker}`);
		}
	}
	referencedModules.add(model.module);
	referencedConfigs.add(model.config);
	workspaces.add(model.workspace);
}

const discovered = [];
const workspaceRunners = [];
for (const file of await readdir(path.join(repositoryRoot, 'formal/tla'), {
	withFileTypes: true,
})) {
	if (file.isFile() && /\.(?:cfg|tla)$/u.test(file.name)) {
		discovered.push(`formal/tla/${file.name}`);
	}
}
for (const parent of ['apps', 'packages']) {
	const workspaceEntries = await readdir(path.join(repositoryRoot, parent), {
		withFileTypes: true,
	});
	for (const workspace of workspaceEntries) {
		if (!workspace.isDirectory()) continue;
		const workspacePath = `${parent}/${workspace.name}`;
		const localRunner = `${workspacePath}/scripts/run-tla.mjs`;
		if (await fileExists(localRunner)) workspaceRunners.push(localRunner);
		const formalDirectory = path.join(repositoryRoot, workspacePath, 'formal/tla');
		let files;
		try {
			files = await readdir(formalDirectory, { withFileTypes: true });
		} catch (error) {
			if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
				continue;
			}
			throw error;
		}
		for (const file of files) {
			if (file.isFile() && /\.(?:cfg|tla)$/u.test(file.name)) {
				discovered.push(`${parent}/${workspace.name}/formal/tla/${file.name}`);
			}
		}
	}
}

if (workspaceRunners.length > 0) {
	throw new Error(
		`Workspace-local TLA+ runners duplicate the shared runner:\n${workspaceRunners.map((file) => `- ${file}`).join('\n')}`,
	);
}

const orphaned = discovered.filter((file) =>
	file.endsWith('.tla') ? !referencedModules.has(file) : !referencedConfigs.has(file),
);
if (orphaned.length > 0) {
	throw new Error(
		`Unregistered TLA+ artifacts:\n${orphaned.map((file) => `- ${file}`).join('\n')}`,
	);
}

for (const workspace of workspaces) {
	if (workspace === '.') continue;
	if (await fileExists(`${workspace}/package.json`)) {
		const packageJson = JSON.parse(await requireFile(`${workspace}/package.json`));
		const command = packageJson.scripts?.['test:tla'];
		if (
			typeof command !== 'string' ||
			!command.includes('scripts/tla/run-models.mjs') ||
			!command.includes(`--workspace ${workspace}`)
		) {
			throw new Error(`${workspace}/package.json must delegate test:tla to the shared TLA+ runner`);
		}
		continue;
	}
	if (!(await fileExists(`${workspace}/Cargo.toml`))) {
		throw new Error(`${workspace} must provide package.json or Cargo.toml for its TLA+ owner`);
	}
}

const rootPackageJson = JSON.parse(await requireFile('package.json'));
validateToolingWorkflow(await requireFile('.github/workflows/verify.yml'));
const toolingModel = inventory.models.find((model) => model.id === toolingModelId);
if (
	toolingModel?.workspace !== '.' ||
	toolingModel.module !== 'formal/tla/TlaVerificationExecution.tla'
)
	throw new Error('TLA inventory must register the root execution tooling model');
if (rootPackageJson.scripts?.['test:tla:tooling'] !== toolingCommand)
	throw new Error('root test:tla:tooling must run the tooling model and real TLC cache witnesses');
const inventorySteps = checkSteps('test:tla:inventory');
for (const file of inventoryTestPaths)
	if (
		!inventorySteps.some(
			(step) =>
				step.command === process.execPath &&
				step.args.includes('--test') &&
				step.args.includes(file),
		)
	)
		throw new Error('Excluded TLA test must run in the mandatory inventory gate: ' + file);
for (const model of inventory.models.filter((entry) => entry.workspace === '.')) {
	if (model.owner !== rootPackageJson.name) {
		throw new Error(`${model.id} must belong to the root package ${rootPackageJson.name}`);
	}
}
if (rootPackageJson.scripts?.['test:tla'] !== 'node scripts/tla/run-models.mjs --all') {
	throw new Error('root test:tla must execute the shared TLA+ runner with --all');
}

process.stdout.write(
	`TLA+ inventory verified: ${inventory.models.length} checks, ${referencedModules.size} modules, ${referencedConfigs.size} configurations\n`,
);
