import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';

import {
	documentScopeChanged,
	validateDocumentScope,
	workflowTlaChanged,
	workflowTlaScope,
	validateToolingWorkflow,
} from './document-scopes.mjs';
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
		triggers: ['apps/orchbee-server/README.md', 'apps/orchbee-server/README.zh-CN.md'],
		documentScopes: ['apps/orchbee-server/README.md', 'apps/orchbee-server/README.zh-CN.md'].map(
			(documentPath) => ({
				path: documentPath,
				relevantHeadings: ['Owned'],
				unrelatedHeadings: ['Other'],
			}),
		),
	})),
};
const scopedIds = scopedInventory.models.map((model) => model.id);
const scopedText = '# Scope fixture\n\n## Owned\nRelevant.\n\n## Other\nUnrelated.\n';

test('each reviewed unrelated section is excluded in both languages beside every relevant positive control', async () => {
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

test('workflow narrowing admits only isolated non-TLC job edits', async () => {
	const before = await readFile(path.join(repositoryRoot, '.github/workflows/verify.yml'), 'utf8');
	validateToolingWorkflow(before);
	for (const invalid of [
		before.replace(
			'node --test scripts/tla/model-cache-cli.test.mjs scripts/tla/model-mutations.test.mjs',
			'echo skipped',
		),
		before.replace('node scripts/tla/select-changed-models.mjs "$TLA_BASE"', 'echo skipped'),
		before.replace(
			'node scripts/tla/run-models.mjs --models "${{ steps.select.outputs.model_ids }}"',
			'echo skipped',
		),
		before.replaceAll("if: steps.select.outputs.count != '0'", 'if: false'),
		before.replace('  tla:', '  tla:\n    if: false'),
		before.replace('  tla:', '  tla:\n    continue-on-error: true'),
		before.replace(
			'        run: node --test scripts/tla/model-cache-cli.test.mjs scripts/tla/model-mutations.test.mjs',
			'        continue-on-error: true\n        run: node --test scripts/tla/model-cache-cli.test.mjs scripts/tla/model-mutations.test.mjs',
		),
	])
		assert.throws(() => validateToolingWorkflow(invalid), /real TLC cache witnesses/u);
	for (const [after, expected] of [
		[before.replace('timeout-minutes: 45', 'timeout-minutes: 46'), 'none'],
		[before.replace('timeout-minutes: 5', 'timeout-minutes: 6'), 'tooling'],
		[before.replace('contents: read', 'contents: write'), 'all'],
		[before.replace('  tla:', '  tla: &shared'), 'all'],
	]) {
		assert.equal(workflowTlaScope(before, after), expected);
		const selected = selectModelsForChangedPaths(
			inventory,
			['.github/workflows/verify.yml'],
			undefined,
			new Map([['.github/workflows/verify.yml', { before, after }]]),
		);
		if (expected === 'tooling')
			assert.deepEqual(
				selected.map(({ id }) => id),
				inventory.models.map(({ id }) => id),
			);
		else assert.equal(selected.length, expected === 'all' ? inventory.models.length : 0);
	}
	assert.equal(
		workflowTlaChanged(before, before.replace('timeout-minutes: 45', 'timeout-minutes: 46')),
		false,
	);
	for (const after of [
		before.replace('timeout-minutes: 5', 'timeout-minutes: 6'),
		before.replace('contents: read', 'contents: write'),
		before.replace('  tla:', '  tla: &shared'),
		before.replace('    name: Change-triggered', '    needs: verify\n    name: Change-triggered'),
		before.replace('  verify:', '  verify: !tag'),
		before.replace('  verify:', '   verify:'),
		`${before}unknown-key: value\n`,
	]) {
		assert.equal(workflowTlaChanged(before, after), true);
	}
	const workflowPath = '.github/workflows/verify.yml';
	assert.deepEqual(
		selectModelsForChangedPaths(
			inventory,
			[workflowPath],
			undefined,
			new Map([
				[
					workflowPath,
					{ before, after: before.replace('timeout-minutes: 45', 'timeout-minutes: 46') },
				],
			]),
		),
		[],
	);
	assert.equal(
		selectModelsForChangedPaths(inventory, [workflowPath]).length,
		inventory.models.length,
	);
});

test('workflow cross-job bindings select conservatively for numeric and quoted YAML forms', async () => {
	const before = await readFile(path.join(repositoryRoot, '.github/workflows/verify.yml'), 'utf8');
	for (const anchor of ['1', '_shared', '共享']) {
		const bound = before
			.replace('  verify:', `  verify:\n    env: &${anchor}\n      VALUE: one`)
			.replace('    name: Change-triggered', `    env: *${anchor}\n    name: Change-triggered`);
		assert.equal(
			workflowTlaChanged(bound, bound.replace('VALUE: one', 'VALUE: two')),
			true,
			anchor,
		);
	}
	for (const dependency of [
		'"needs": verify',
		"'needs': verify",
		'env: { VALUE: ${{ needs["verify"].outputs.value }} }',
		"env: { VALUE: ${{ jobs['verify'].outputs.value }} }",
	]) {
		const bound = before.replace(
			'    name: Change-triggered',
			`    ${dependency}\n    name: Change-triggered`,
		);
		assert.equal(
			workflowTlaChanged(bound, bound.replace('timeout-minutes: 45', 'timeout-minutes: 46')),
			true,
			dependency,
		);
	}
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
