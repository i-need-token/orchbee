import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';

import { documentScopeChanged, validateDocumentScope } from './document-scopes.mjs';
import {
	inventoryPath,
	repositoryRoot,
	selectModelsForChangedPaths,
	validateInventory,
} from './model-inventory.mjs';

const inventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
const scopedInventory = {
	...inventory,
	models: inventory.models.slice(0, 3).map((model) => ({
		...model,
		claims: ['apps/orchbee-server/README.md'],
		triggers: ['apps/orchbee-server/README.md'],
		documentScopes: [
			{
				path: 'apps/orchbee-server/README.md',
				relevantHeadings: ['Owned'],
				unrelatedHeadings: ['Other'],
			},
		],
	})),
};
const scopedIds = scopedInventory.models.map((model) => model.id);
const scopedText = '# Scope fixture\n\n## Owned\nRelevant.\n\n## Other\nUnrelated.\n';

test('each reviewed unrelated section is excluded beside every relevant positive control', async () => {
	for (const id of scopedIds) {
		const model = scopedInventory.models.find((entry) => entry.id === id);
		for (const scope of model.documentScopes) {
			const before = scopedText;
			validateDocumentScope(scope, before);
			for (const [headings, expected] of [
				[scope.unrelatedHeadings, false],
				[scope.relevantHeadings, true],
			]) {
				for (const heading of headings) {
					const after = before.replace(
						`## ${heading}`,
						`## ${heading}\n\nChanged contract paragraph.`,
					);
					const selected = selectModelsForChangedPaths(
						scopedInventory,
						[scope.path],
						undefined,
						new Map([[scope.path, { before, after }]]),
					);
					assert.equal(
						selected.some((entry) => entry.id === id),
						expected,
						`${id}: ${scope.path}: ${heading}`,
					);
				}
			}
			assert.ok(
				selectModelsForChangedPaths(scopedInventory, [scope.path]).some((entry) => entry.id === id),
			);
		}
	}
});

test('shared preambles, reference definitions and unsupported headings conservatively select', () => {
	const scope = { path: 'README.md', relevantHeadings: ['Owned'], unrelatedHeadings: ['Other'] };
	const before = '# Title\n\n## Owned\nStable.\n\n## Other\nOther.\n';
	assert.equal(documentScopeChanged(scope, before, before.replace('Other.', 'Changed.')), false);
	for (const after of [
		before.replace('Title', 'Changed'),
		`${before}[source]: https://example.test\n`,
		`${before}## New\n`,
		`${before}## Owned\n`,
		`${before}~~~\n`,
		before.replace('## Other', 'Other\n====='),
		before.replace('## Other', ' ## Other'),
	]) {
		assert.equal(documentScopeChanged(scope, before, after), true, after);
	}
	assert.throws(() => validateDocumentScope(scope, `${before}## New\n`), /classify every/u);
	assert.throws(
		() => validateDocumentScope({ ...scope, unrelatedHeadings: ['Owned'] }, before),
		/classify every/u,
	);
	const fenced = before.replace('Stable.', '~~~md\n## Fake\n~~~\nStable.');
	validateDocumentScope(scope, fenced);
	assert.equal(documentScopeChanged(scope, fenced, fenced), false);
});

test('scope metadata rejects unknown fields, unregistered dependencies and overlapping headings', () => {
	for (const mutate of [
		(entry) => {
			entry.documentScopes[0].future = true;
		},
		(entry) => {
			entry.documentScopes[0].path = 'other.md';
		},
		(entry) => {
			entry.documentScopes[0].unrelatedHeadings.push(entry.documentScopes[0].relevantHeadings[0]);
		},
	]) {
		const changed = structuredClone(scopedInventory);
		mutate(changed.models.find((entry) => entry.id === scopedIds[0]));
		assert.throws(() => validateInventory(changed));
	}
	assert.throws(
		() => validateInventory({ ...inventory, future: true }),
		/unknown execution metadata/u,
	);
});

test('Git selection retains both rename paths and old/new scopes without dropping blank lines', async (context) => {
	const root = await mkdtemp(path.join(os.tmpdir(), 'orchbee-tla-selector-test-'));
	context.after(() => rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }));
	await mkdir(path.join(root, 'scripts/tla'), { recursive: true });
	for (const file of ['model-inventory.mjs', 'document-scopes.mjs'])
		await cp(path.join(repositoryRoot, 'scripts/tla', file), path.join(root, 'scripts/tla', file));
	const fixture = structuredClone(scopedInventory);
	fixture.models = fixture.models.filter((entry) => scopedIds.includes(entry.id));
	await writeFile(path.join(root, 'scripts/tla/models.json'), JSON.stringify(fixture));
	await mkdir(path.join(root, 'apps/orchbee-server'), { recursive: true });
	for (const scope of fixture.models[0].documentScopes)
		await writeFile(path.join(root, scope.path), scopedText);
	const git = (...args) => {
		const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
		assert.equal(result.status, 0, result.stderr);
		return result.stdout.trim();
	};
	git('init', '-b', 'main');
	git('config', 'user.name', 'Selector test');
	git('config', 'user.email', 'selector@example.test');
	git('add', '.');
	git('commit', '-m', 'fixture');
	const { selectModelsSince } = await import(
		pathToFileURL(path.join(root, 'scripts/tla/model-inventory.mjs'))
	);
	const scope = fixture.models[0].documentScopes[0];
	const before = await readFile(path.join(root, scope.path), 'utf8');
	await writeFile(
		path.join(root, scope.path),
		before.replace(
			`## ${scope.unrelatedHeadings[0]}`,
			`## ${scope.unrelatedHeadings[0]}\n\nUnrelated.`,
		),
	);
	assert.deepEqual(await selectModelsSince(fixture, 'HEAD'), []);
	await writeFile(
		path.join(root, scope.path),
		before.replace(`## ${scope.relevantHeadings[0]}`, `## ${scope.relevantHeadings[0]}\n\n`),
	);
	assert.equal((await selectModelsSince(fixture, 'HEAD')).length, 3);
	await writeFile(path.join(root, scope.path), before);
	git('mv', scope.path, 'apps/orchbee-server/RENAMED.md');
	assert.equal((await selectModelsSince(fixture, 'HEAD')).length, 3);
	await assert.rejects(selectModelsSince(fixture, 'missing-base'));
});
