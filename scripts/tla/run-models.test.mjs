import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { describe } from 'node:test';
import { runChild } from './test-support/child-process.mjs';
import { pathToFileURL } from 'node:url';

describe('isolated TLC CLI fixtures', { concurrency: 6 }, () => {
	// Model the pinned resolver's fixed module filename and deletion on JVM exit. IPC orders the
	// sibling's exit before the first reader resumes, so missing isolation fails deterministically.
	const childSource = `
import assert from 'node:assert/strict';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { writeSync } from 'node:fs';
import path from 'node:path';
const { directory, identity } = JSON.parse(process.argv[2]);
await mkdir(directory, { recursive: true });
const modulePath = path.join(directory, 'Naturals.tla');
await writeFile(modulePath, identity);
process.on('message', async (command) => {
    try {
        if (command === 'verify') assert.equal(await readFile(modulePath, 'utf8'), identity);
        await rm(modulePath);
        writeSync(1, '<Advance line 1, col 1 of module Example>: 1:2\\n');
        process.exit(0);
    } catch (error) {
        writeSync(2, String(error));
        process.exit(1);
    }
});
process.send('ready');
`;

	const preloadSource = `
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.dirname(fileURLToPath(import.meta.url));
const spawn = childProcess.spawn;
const children = [];
let ready = 0;
childProcess.spawn = (_java, args, options) => {
    const jvmOptions = args.slice(0, args.indexOf('-jar'));
    const prefix = '-Djava.io.tmpdir=';
    const directory = jvmOptions.find((argument) => argument.startsWith(prefix))?.slice(prefix.length)
        ?? path.join(root, 'shared JVM temp');
    const child = spawn(process.execPath, [path.join(root, 'java-child.mjs'), JSON.stringify({
        directory, identity: String(children.length),
    })], { ...options, stdio: [...options.stdio, 'ipc'] });
    children.push(child);
    child.on('message', () => {
        ready += 1;
        if (ready === 2) {
            children[1].once('close', () => children[0].send('verify'));
            children[1].send('exit');
        }
    });
    return child;
};
syncBuiltinESMExports();
`;

	test('concurrent variants retain private modules after a sibling exits and clean both directories', async () => {
		const root = mkdtempSync(path.join(tmpdir(), 'orchbee TLC runner isolation '));
		const scripts = path.join(root, 'scripts/tla');
		const formal = path.join(root, 'scripts/formal/tla');
		const tools = path.join(root, '.tools');
		try {
			mkdirSync(scripts, { recursive: true });
			mkdirSync(formal, { recursive: true });
			mkdirSync(tools);
			for (const filename of [
				'run-models.mjs',
				'model-checks.mjs',
				'toolchain.mjs',
				'model-inventory.mjs',
				'document-scopes.mjs',
				'model-inputs.mjs',
				'model-records.mjs',
				'model-cache.mjs',
				'model-cache-identity.mjs',
			]) {
				copyFileSync(new URL(filename, import.meta.url), path.join(scripts, filename));
			}
			const jar = 'test JVM adapter';
			writeFileSync(path.join(tools, 'tla2tools-1.0.0.jar'), jar);
			writeFileSync(path.join(formal, 'Example.tla'), '---- MODULE Example ----\n====\n');
			writeFileSync(path.join(formal, 'Example.cfg'), 'CONSTANT Bound = 1\n');
			writeFileSync(
				path.join(scripts, 'models.json'),
				JSON.stringify({
					schemaVersion: 1,
					defaultTimeoutMilliseconds: 10_000,
					sharedTriggers: ['scripts/tla/**'],
					tool: {
						version: '1.0.0',
						downloadUrl: 'https://example.test/unused.jar',
						sha256: createHash('sha256').update(jar).digest('hex'),
					},
					models: [
						{
							id: 'example',
							owner: 'example',
							workspace: 'scripts',
							module: 'scripts/formal/tla/Example.tla',
							config: 'scripts/formal/tla/Example.cfg',
							claims: ['docs/VERIFICATION.md'],
							triggers: ['scripts/tla/**'],
							requiredCoverage: ['Advance'],
							variants: [
								{ name: 'baseline', constants: { Bound: 1 } },
								{ name: 'changed', constants: { Bound: 2 } },
							],
						},
					],
				}),
			);
			writeFileSync(path.join(root, 'java-child.mjs'), childSource);
			const preload = path.join(root, 'java-adapter.mjs');
			writeFileSync(preload, preloadSource);
			const env = { ...process.env };
			delete env.ORCHBEE_TLA2TOOLS_JAR;
			delete env.ORCHBEE_TLA_TIMEOUT_MILLISECONDS;
			const result = await runChild(
				process.execPath,
				[
					'--import',
					pathToFileURL(preload).href,
					path.join(scripts, 'run-models.mjs'),
					'--all',
					'--jobs',
					'2',
				],
				{ env, encoding: 'utf8', timeout: 20_000 },
			);
			assert.ifError(result.error);
			assert.equal(result.status, 0, result.stdout + result.stderr);
			assert.match(result.stdout, /Checked 2 TLA\+ model configurations/u);
			assert.equal(result.stdout.match(/^TLA_RESULT /gmu)?.length, 2);
			const reports = result.stdout
				.split('\n')
				.filter((line) => line.startsWith('TLA_RESULT '))
				.map((line) => JSON.parse(line.slice('TLA_RESULT '.length)));
			for (const directory of new Set(reports.map((report) => path.dirname(report.logPath))))
				rmSync(directory, { recursive: true, force: true });
			assert.deepEqual(readdirSync(tools), ['tla2tools-1.0.0.jar']);
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	});

	function createRunnerFixture(t) {
		const root = mkdtempSync(path.join(tmpdir(), 'orchbee TLC outcome '));
		t.after(() => rmSync(root, { recursive: true, force: true }));
		const scripts = path.join(root, 'scripts/tla');
		const tools = path.join(root, '.tools');
		mkdirSync(scripts, { recursive: true });
		mkdirSync(tools);
		for (const name of [
			'run-models.mjs',
			'model-checks.mjs',
			'toolchain.mjs',
			'model-inventory.mjs',
			'document-scopes.mjs',
			'model-inputs.mjs',
			'model-records.mjs',
			'model-cache.mjs',
			'model-cache-identity.mjs',
			'select-changed-models.mjs',
		]) {
			copyFileSync(new URL(name, import.meta.url), path.join(scripts, name));
		}
		const jar = 'test JVM adapter';
		writeFileSync(path.join(tools, 'tla2tools-1.0.0.jar'), jar);
		const inventory = {
			schemaVersion: 1,
			defaultTimeoutMilliseconds: 10_000,
			sharedTriggers: ['scripts/tla/**'],
			tool: {
				version: '1.0.0',
				downloadUrl: 'https://example.test/unused.jar',
				sha256: createHash('sha256').update(jar).digest('hex'),
			},
			models: ['alpha', 'beta'].map((id) => {
				const formal = `apps/${id}/formal/tla`;
				mkdirSync(path.join(root, formal), { recursive: true });
				writeFileSync(path.join(root, formal, 'Example.tla'), '---- MODULE Example ----\n====\n');
				writeFileSync(path.join(root, formal, 'Example.cfg'), 'CONSTANT Bound = 1\n');
				writeFileSync(
					path.join(root, `apps/${id}/README.md`),
					`# ${id}\nCurrent claim authority.\n`,
				);
				return {
					id,
					owner: id,
					workspace: `apps/${id}`,
					module: `${formal}/Example.tla`,
					config: `${formal}/Example.cfg`,
					claims: [`apps/${id}/README.md`],
					triggers: [`apps/${id}/src/**`],
					requiredCoverage: ['Advance'],
				};
			}),
		};
		const inventoryFile = path.join(scripts, 'models.json');
		writeFileSync(inventoryFile, JSON.stringify(inventory));
		const env = { ...process.env };
		delete env.ORCHBEE_TLA2TOOLS_JAR;
		delete env.ORCHBEE_TLA_TIMEOUT_MILLISECONDS;
		const invoke = (entry, args, extra = {}) =>
			runChild(process.execPath, [path.join(scripts, entry), ...args], {
				cwd: root,
				env: { ...env, ...extra },
				encoding: 'utf8',
				timeout: 20_000,
			});
		return { root, scripts, tools, inventory, inventoryFile, invoke };
	}

	test('CLI rejects invalid heap ceilings before provisioning or publishing a run', async (t) => {
		const f = createRunnerFixture(t);
		const directory = path.join(f.root, 'uncreated-run');
		const extra = { ORCHBEE_TLA2TOOLS_JAR: path.join(f.root, 'unavailable.jar') };
		const missing = await f.invoke('run-models.mjs', ['--all', '--max-heap-mib'], extra);
		assert.notEqual(missing.status, 0);
		assert.match(missing.stderr, /--max-heap-mib requires a value/u);
		for (const value of [
			'',
			'0',
			'255',
			'8193',
			'1.5',
			'NaN',
			'Infinity',
			'4096.0',
			' 4096',
			'+4096',
			'04096',
			'1e3',
			'9007199254740992',
		]) {
			const result = await f.invoke(
				'run-models.mjs',
				['--all', '--max-heap-mib', value, '--run-dir', directory],
				extra,
			);
			assert.notEqual(result.status, 0);
			assert.match(result.stderr, /--max-heap-mib must be an integer between 256 and 8192/u);
			assert.ok(!existsSync(directory));
		}
	});

	test('CLI reads the requested Git inventory base and fails safe for missing or malformed bases', async (t) => {
		const fixture = createRunnerFixture(t);
		function git(...args) {
			const result = spawnSync('git', args, { cwd: fixture.root, encoding: 'utf8' });
			assert.equal(result.status, 0, result.stderr);
			return result.stdout.trim();
		}
		git('init', '--quiet');
		git('add', '.');
		git(
			'-c',
			'user.name=TLC test',
			'-c',
			'user.email=tlc@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'base',
		);
		const base = git('rev-parse', 'HEAD');
		fixture.inventory.models[0].workers = 2;
		writeFileSync(fixture.inventoryFile, JSON.stringify(fixture.inventory));
		const scoped = await fixture.invoke('select-changed-models.mjs', [base]);
		assert.equal(scoped.status, 0, scoped.stderr);
		assert.equal(scoped.stdout, 'alpha\n');
		const unknown = await fixture.invoke('select-changed-models.mjs', ['missing-reference']);
		assert.equal(unknown.status, 0);
		assert.equal(unknown.stdout, 'alpha,beta\n');
		assert.match(unknown.stderr, /selecting all/u);
		const source = path.join(fixture.root, 'apps/beta/src');
		mkdirSync(source);
		for (const name of process.platform === 'win32'
			? ['未跟踪.rs']
			: ['未跟踪.rs', 'line\nbreak.rs']) {
			const file = path.join(source, name);
			writeFileSync(file, 'changed\n');
			const unicode = await fixture.invoke('select-changed-models.mjs', [base]);
			assert.equal(unicode.status, 0, unicode.stderr);
			assert.equal(unicode.stdout, 'alpha,beta\n');
			rmSync(file);
		}
		const unrelatedCommit = git(
			'-c',
			'user.name=TLC test',
			'-c',
			'user.email=tlc@example.invalid',
			'commit-tree',
			'HEAD^{tree}',
			'-m',
			'unrelated root',
		);
		for (const invalid of [unrelatedCommit, 'HEAD:apps/alpha/README.md', '-M']) {
			const invalidBase = await fixture.invoke('select-changed-models.mjs', [invalid]);
			assert.equal(invalidBase.status, 0, invalidBase.stderr);
			assert.equal(invalidBase.stdout, 'alpha,beta\n');
			assert.match(invalidBase.stderr, /selecting all/u);
		}
		for (const flag of ['assume-unchanged', 'skip-worktree']) {
			git('update-index', '--' + flag, 'apps/alpha/README.md');
			const hidden = await fixture.invoke('select-changed-models.mjs', [base]);
			assert.equal(hidden.stdout, 'alpha,beta\n');
			assert.match(hidden.stderr, /selecting all/u);
			git('update-index', '--no-' + flag, 'apps/alpha/README.md');
		}
		writeFileSync(fixture.inventoryFile, 'not valid JSON');
		git('add', '.');
		git(
			'-c',
			'user.name=TLC test',
			'-c',
			'user.email=tlc@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'invalid inventory',
		);
		const malformed = git('rev-parse', 'HEAD');
		writeFileSync(fixture.inventoryFile, JSON.stringify(fixture.inventory));
		const fallback = await fixture.invoke('select-changed-models.mjs', [malformed]);
		assert.equal(fallback.status, 0);
		assert.equal(fallback.stdout, 'alpha,beta\n');
		assert.match(fallback.stderr, /selecting all/u);
	});

	test('Git selection preserves staged source, document, workflow and inventory changes hidden by the worktree', async (t) => {
		const f = createRunnerFixture(t);
		const git = (...args) => {
			const result = spawnSync('git', ['-c', 'core.autocrlf=false', ...args], {
				cwd: f.root,
				encoding: 'utf8',
			});
			assert.equal(result.status, 0, result.stderr);
			return result.stdout.trim();
		};
		const source = 'apps/alpha/src/masked.rs';
		mkdirSync(path.dirname(path.join(f.root, source)), { recursive: true });
		writeFileSync(path.join(f.root, source), 'base\n');
		const claim = 'apps/alpha/README.md';
		const claimText = '# alpha\n\n## Owned\nRelevant.\n\n## Other\nOther.\n';
		writeFileSync(path.join(f.root, claim), claimText);
		f.inventory.models[0].documentScopes = [
			{ path: claim, relevantHeadings: ['Owned'], unrelatedHeadings: ['Other'] },
		];
		const workflow = '.github/workflows/verify.yml';
		const workflowText =
			'name: Verify\njobs:\n  verify:\n    timeout-minutes: 1\n  tla:\n    runs-on: ubuntu-latest\n';
		mkdirSync(path.dirname(path.join(f.root, workflow)), { recursive: true });
		writeFileSync(path.join(f.root, workflow), workflowText);
		mkdirSync(path.join(f.root, 'formal/tla'), { recursive: true });
		writeFileSync(path.join(f.root, 'formal/tla/Tool.tla'), '---- MODULE Tool ----\n====\n');
		writeFileSync(path.join(f.root, 'formal/tla/Tool.cfg'), 'CONSTANT Bound = 1\n');
		f.inventory.models.unshift({
			...f.inventory.models[1],
			id: 'tla-verification-execution',
			owner: 'fixture',
			workspace: '.',
			module: 'formal/tla/Tool.tla',
			config: 'formal/tla/Tool.cfg',
			triggers: ['scripts/tla/**'],
		});
		const inventoryText = JSON.stringify(f.inventory);
		writeFileSync(f.inventoryFile, inventoryText);
		git('init', '--quiet');
		git('add', '.');
		git(
			'-c',
			'user.name=Fixture',
			'-c',
			'user.email=fixture@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'base',
		);
		const base = git('rev-parse', 'HEAD');
		const cases = [
			[source, 'base\n', 'staged\n', 'alpha\n'],
			[claim, claimText, claimText.replace('Relevant.', 'Changed.'), 'alpha\n'],
			[claim, claimText, claimText.replace('Other.', 'Changed.'), '\n'],
			[
				workflow,
				workflowText,
				workflowText.replace('ubuntu-latest', 'windows-latest'),
				'tla-verification-execution,alpha,beta\n',
			],
			[
				workflow,
				workflowText,
				workflowText.replace('timeout-minutes: 1', 'timeout-minutes: 2'),
				'\n',
			],
			[
				'scripts/tla/models.json',
				inventoryText,
				JSON.stringify({
					...f.inventory,
					models: f.inventory.models.map((m) => (m.id === 'alpha' ? { ...m, workers: 2 } : m)),
				}),
				'tla-verification-execution,alpha,beta\n',
			],
		];
		for (const [file, original, staged, expected] of cases) {
			writeFileSync(path.join(f.root, file), staged);
			git('add', '--', file);
			writeFileSync(path.join(f.root, file), original);
			const selected = await f.invoke('select-changed-models.mjs', [base]);
			assert.equal(selected.status, 0, selected.stderr);
			assert.equal(selected.stdout, expected, file);
			git('restore', '--staged', '--', file);
		}
		writeFileSync(path.join(f.root, claim), claimText.replace('Relevant.', 'Committed change.'));
		git('add', '--', claim);
		git(
			'-c',
			'user.name=Fixture',
			'-c',
			'user.email=fixture@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'claim',
		);
		writeFileSync(path.join(f.root, claim), claimText);
		git('add', '--', claim);
		const committedThenReverted = await f.invoke('select-changed-models.mjs', [base]);
		assert.equal(committedThenReverted.status, 0, committedThenReverted.stderr);
		assert.equal(committedThenReverted.stdout, 'alpha\n');
	});

	test('Git source exclusions cannot hide same-diff edits and require tracked existing files', async (t) => {
		const f = createRunnerFixture(t);
		const git = (...args) => {
			const result = spawnSync('git', ['-c', 'core.autocrlf=false', ...args], {
				cwd: f.root,
				encoding: 'utf8',
			});
			assert.equal(result.status, 0, result.stderr);
			return result.stdout.trim();
		};
		const commit = () => {
			git('add', '.');
			git(
				'-c',
				'user.name=Fixture',
				'-c',
				'user.email=fixture@example.invalid',
				'commit',
				'--quiet',
				'-m',
				'fixture',
			);
			return git('rev-parse', 'HEAD');
		};
		const excluded = 'apps/alpha/src/presentation.rs';
		mkdirSync(path.join(f.root, 'apps/alpha/src'));
		writeFileSync(path.join(f.root, excluded), 'presentation\n');
		mkdirSync(path.join(f.root, 'formal/tla'), { recursive: true });
		writeFileSync(path.join(f.root, 'formal/tla/Tool.tla'), '---- MODULE Tool ----\n====\n');
		writeFileSync(path.join(f.root, 'formal/tla/Tool.cfg'), 'CONSTANT Bound = 1\n');
		f.inventory.models.unshift({
			...f.inventory.models[0],
			id: 'tla-verification-execution',
			owner: 'fixture',
			workspace: '.',
			module: 'formal/tla/Tool.tla',
			config: 'formal/tla/Tool.cfg',
			triggers: ['scripts/tla/**'],
		});
		writeFileSync(f.inventoryFile, JSON.stringify(f.inventory));
		writeFileSync(
			path.join(f.scripts, 'exclusions-witness.mjs'),
			"import {assertTriggerExclusionAdmission} from './model-inventory.mjs'; await assertTriggerExclusionAdmission(process.argv.slice(2));\n",
		);
		git('init', '--quiet');
		const base = commit();
		f.inventory.models[1].triggerExclusions = [excluded];
		writeFileSync(f.inventoryFile, JSON.stringify(f.inventory));
		let selected = await f.invoke('select-changed-models.mjs', [base]);
		assert.equal(selected.status, 0, selected.stderr);
		assert.equal(selected.stdout, 'tla-verification-execution,alpha,beta\n');
		writeFileSync(path.join(f.root, excluded), 'changed presentation\n');
		selected = await f.invoke('select-changed-models.mjs', [base]);
		assert.equal(selected.stdout, 'tla-verification-execution,alpha,beta\n');
		const narrowed = commit();
		writeFileSync(path.join(f.root, excluded), 'another presentation change\n');
		selected = await f.invoke('select-changed-models.mjs', [narrowed]);
		assert.equal(selected.stdout, '\n');
		assert.equal((await f.invoke('exclusions-witness.mjs', [excluded])).status, 0);
		const untracked = 'apps/alpha/src/untracked.rs';
		writeFileSync(path.join(f.root, untracked), 'untracked\n');
		const invalid = await f.invoke('exclusions-witness.mjs', [untracked]);
		assert.notEqual(invalid.status, 0);
		assert.match(invalid.stderr, /exclusion must be tracked/u);
		rmSync(path.join(f.root, excluded));
		const stale = await f.invoke('exclusions-witness.mjs', [excluded]);
		assert.notEqual(stale.status, 0);
		assert.match(stale.stderr, /ENOENT/u);
	});

	const outcomeChild = `
import { writeSync } from 'node:fs';
writeSync(1, '7 states generated, 3 distinct states found, 2 states left on queue\\n');
if (process.env.TLC_TEST_OUTCOME === 'timeout') setInterval(() => {}, 1000);
else {
    writeSync(2, 'counterexample witness\\n');
    process.exit(7);
}
`;

	const outcomePreload = `
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
import { fileURLToPath } from 'node:url';
const spawn = childProcess.spawn;
childProcess.spawn = (_java, args, options) => {
    assert.ok(args.some((argument) => /^-Xmx[0-9]+m$/.test(argument)));
    return spawn(process.execPath, [fileURLToPath(new URL('outcome-child.mjs', import.meta.url))], options);
};
syncBuiltinESMExports();
`;

	for (const outcome of ['failed', 'timeout']) {
		test(`runner records ${outcome}, retains its log, and identifies unstarted work without claiming success`, async (t) => {
			const fixture = createRunnerFixture(t);
			writeFileSync(path.join(fixture.root, 'outcome-child.mjs'), outcomeChild);
			const preload = path.join(fixture.root, 'outcome-preload.mjs');
			writeFileSync(preload, outcomePreload);
			const result = await fixture.invoke('run-models.mjs', ['--all', '--jobs', '1'], {
				NODE_OPTIONS: `--import=${pathToFileURL(preload).href}`,
				TLC_TEST_OUTCOME: outcome,
				ORCHBEE_TLA_TIMEOUT_MILLISECONDS: '1000',
			});
			assert.ifError(result.error);
			assert.notEqual(result.status, 0);
			assert.doesNotMatch(result.stdout, /^TLA_RESULT /mu);
			const lines = result.stdout.split('\n');
			const report = JSON.parse(
				lines.find((line) => line.startsWith('TLA_FAILURE ')).slice('TLA_FAILURE '.length),
			);
			t.after(() => rmSync(path.dirname(report.logPath), { recursive: true, force: true }));
			assert.equal(report.status, outcome);
			assert.equal(report.model, 'alpha');
			assert.equal(report.statesLeft, 2);
			assert.ok(report.durationMilliseconds > 0);
			assert.ok(existsSync(report.logPath));
			assert.match(readFileSync(report.logPath, 'utf8'), /7 states generated/u);
			if (outcome === 'failed') assert.equal(report.exitCode, 7);
			else assert.equal(report.signal, 'SIGTERM');
			const skipped = JSON.parse(
				lines.find((line) => line.startsWith('TLA_SKIPPED ')).slice('TLA_SKIPPED '.length),
			);
			assert.equal(skipped.model, 'beta');
			assert.equal(skipped.reason, 'earlier-failure');
			assert.deepEqual(readdirSync(fixture.tools), ['tla2tools-1.0.0.jar']);
		});
	}

	function freezeFixture(fixture) {
		const git = (...args) => {
			const result = spawnSync('git', args, { cwd: fixture.root, encoding: 'utf8' });
			assert.equal(result.status, 0, result.stderr);
		};
		git('init', '--quiet');
		git('add', '.');
		git(
			'-c',
			'user.name=TLC test',
			'-c',
			'user.email=tlc@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'frozen fixture',
		);
	}

	test('child exit closes admission while its stdout remains open and another active child finishes', async (t) => {
		const f = createRunnerFixture(t);
		const gamma = {
			...f.inventory.models[0],
			id: 'gamma',
			owner: 'gamma',
			workspace: 'apps/gamma',
			claims: ['apps/gamma/README.md'],
			triggers: ['apps/gamma/src/**'],
		};
		f.inventory.models.push(gamma);
		for (const model of f.inventory.models) {
			model.workers = 1;
			const formal = `apps/${model.id}/formal/tla`;
			mkdirSync(path.join(f.root, formal), { recursive: true });
			model.module = `${formal}/${model.id}.tla`;
			model.config = `${formal}/${model.id}.cfg`;
			writeFileSync(path.join(f.root, model.module), `---- MODULE ${model.id} ----\n====\n`);
			writeFileSync(path.join(f.root, model.config), 'CONSTANT Bound = 1\n');
		}
		writeFileSync(f.inventoryFile, JSON.stringify(f.inventory));
		writeFileSync(
			path.join(f.root, 'drain-child.mjs'),
			`
import { writeSync } from 'node:fs';
const role = process.argv[2];
if (role === 'alpha') {
 process.send('ready');
 process.on('message', () => process.exit(7));
} else {
 const finish = () => { writeSync(1, 'Model checking completed. No error has been found.\\n2 states generated, 1 distinct states found, 0 states left on queue.\\nFinished in 1s\\n<Advance line 1>: 1:2\\n'); process.exit(0); };
 if (role === 'beta') { process.on('message', finish); process.send('ready'); }
 else finish();
}
`,
		);
		const preload = path.join(f.root, 'drain-preload.mjs');
		writeFileSync(
			preload,
			`
import childProcess from 'node:child_process';
import { appendFileSync, writeSync } from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
os.availableParallelism = () => 2;
os.totalmem = () => 20 * 1024 ** 3;
const root = path.dirname(fileURLToPath(import.meta.url));
const trace = (event) => appendFileSync(path.join(root, 'drain-events.txt'), event + '\\n');
const spawn = childProcess.spawn;
const children = {};
const ready = new Set();
childProcess.spawn = (command, args, options) => {
 if (command !== 'test-java') return spawn(command, args, options);
 const role = path.basename(args.at(-1), '.tla');
 trace('spawn-' + role);
 const child = spawn(process.execPath, [path.join(root, 'drain-child.mjs'), role], { ...options, stdio: ['ignore', 'pipe', 'pipe', 'ipc'] });
 // Real Node children supply exits; the Java adapter controls delayed stdio closure portably.
 const emit = child.emit;
 child.emit = function(event, ...values) {
  if (event === 'close' && role === 'alpha') { setTimeout(() => emit.call(this, event, ...values), 500); return true; }
  return emit.call(this, event, ...values);
 };
 child.stdout.on('data', (chunk) => writeSync(options.stdio[1], chunk));
 child.stderr.on('data', (chunk) => writeSync(options.stdio[2], chunk));
 children[role] = child;
 child.once('message', () => {
  ready.add(role);
  if (ready.has('alpha') && ready.has('beta')) children.alpha.send('exit');
 });
 child.once('exit', () => {
  trace('exit-' + role);
  if (role === 'alpha') children.beta.send('finish');
 });
 child.once('close', () => trace('close-' + role));
 return child;
};
syncBuiltinESMExports();
`,
		);
		const result = await f.invoke('run-models.mjs', ['--all', '--jobs', '2'], {
			ORCHBEE_TLA_JAVA: 'test-java',
			NODE_OPTIONS: `--import=${pathToFileURL(preload).href}`,
		});
		const logRoot = /^Logs: (.+)$/mu.exec(result.stdout)?.[1].trim();
		if (logRoot) t.after(() => rmSync(logRoot, { recursive: true, force: true }));
		assert.ifError(result.error);
		assert.notEqual(result.status, 0, result.stdout + result.stderr);
		const events = readFileSync(path.join(f.root, 'drain-events.txt'), 'utf8').trim().split('\n');
		assert.ok(events.indexOf('exit-alpha') < events.indexOf('close-beta'), events.join(','));
		assert.ok(events.indexOf('close-beta') < events.indexOf('close-alpha'), events.join(','));
		assert.ok(!events.includes('spawn-gamma'), events.join(','));
		assert.match(result.stdout, /TLA_SKIPPED .*"model":"gamma"/u);
		assert.deepEqual(readdirSync(f.tools), ['tla2tools-1.0.0.jar']);
	});

	const continuationChild = `
import { writeSync } from 'node:fs';
if (process.argv[2] === 'version' || process.argv[2] === 'slow-version') {
 if (process.argv[2] === 'slow-version') await new Promise((resolve) => setTimeout(resolve, 500));
 writeSync(2, 'test Java 17 identity\\n'); process.exit(0);
}
const outcome = process.env.TLC_TEST_OUTCOME;
if (outcome === 'failed') { writeSync(2, 'original failure\\n'); process.exit(7); }
if (outcome === 'cancel') setInterval(() => {}, 1000);
else {
 if (outcome !== 'incomplete') writeSync(1, 'Model checking completed. No error has been found.\\n2 states generated, 1 distinct states found, 0 states left on queue.\\nThe depth of the complete state graph search is 1.\\nFinished in 1s\\n');
 writeSync(1, '<Advance line 1, col 1 of module Example>: 1:2\\n');
}
`;

	const continuationPreload = `
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import fsPromises from 'node:fs/promises';
import { syncBuiltinESMExports } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
if (process.env.TLC_TEST_MEMORY_GIB) {
 os.totalmem = () => Number(process.env.TLC_TEST_MEMORY_GIB) * 1024 ** 3;
 process.constrainedMemory = () => 0;
}
const spawn = childProcess.spawn;
let versionProbes = 0;
const open = fsPromises.open;
fsPromises.open = async (target, ...options) => {
 const file = await open(target, ...options);
 if (path.basename(target) === '.tla-run.lock' && process.env.TLC_TEST_OUTCOME === 'cancel-acquire') {
  if (process.platform === 'win32') process.emit('SIGTERM');
  else process.kill(process.pid, 'SIGTERM');
  await new Promise((resolve) => setTimeout(resolve, 30));
 }
 return file;
};
const rm = fsPromises.rm;
const writeFile = fsPromises.writeFile;
fsPromises.writeFile = async (target, ...options) => {
 const result = await writeFile(target, ...options);
 const recordDirectory = path.basename(path.dirname(target));
 const outcome = process.env.TLC_TEST_OUTCOME;
 if (String(target).endsWith('.tmp') && ((recordDirectory === 'records' && outcome === 'cancel-config-write') || (recordDirectory === 'models' && outcome === 'cancel-model-write'))) {
  process.emit('SIGTERM');
  await new Promise((resolve) => setImmediate(resolve));
 }
 return result;
};
fsPromises.rm = async (target, options) => {
 if (path.basename(target) === '.tla-run.lock' && process.env.TLC_TEST_OUTCOME === 'cancel-final-close') {
  process.emit('SIGTERM');
  await new Promise((resolve) => setTimeout(resolve, 20));
 }
 return rm(target, options);
};
childProcess.spawn = (command, args, options) => {
 if (command !== 'test-java') return spawn(command, args, options);
 const version = args.includes('-version');
 const slowVersion = version && ++versionProbes === 2 && process.env.TLC_TEST_OUTCOME === 'cancel-final-probe';
 if (slowVersion) setTimeout(() => process.emit('SIGTERM'), 100);
 if (!version) {
  if (process.env.TLC_TEST_HEAP_MIB) assert.ok(args.includes('-Xmx' + process.env.TLC_TEST_HEAP_MIB + 'm'));
  assert.match(readFileSync(path.join(options.cwd, args.at(-1)), 'utf8'), /MODULE Example/);
  assert.match(readFileSync(path.join(options.cwd, args[args.indexOf('-config') + 1]), 'utf8'), /CONSTANT Bound = 1/);
  if (process.env.TLC_TEST_OUTCOME === 'cancel') setTimeout(() => process.emit('SIGTERM'), 100);
  if (['dirty-claim', 'commit-claim'].includes(process.env.TLC_TEST_OUTCOME)) {
   const root = path.dirname(fileURLToPath(import.meta.url));
   writeFileSync(path.join(root, 'apps/alpha/README.md'), '# alpha\\nChanged current claim.\\n');
   if (process.env.TLC_TEST_OUTCOME === 'commit-claim') {
    for (const command of [['add', 'apps/alpha/README.md'], ['-c', 'user.name=TLC test', '-c', 'user.email=tlc@example.invalid', 'commit', '--quiet', '-m', 'concurrent revision']]) {
     const result = childProcess.spawnSync('git', command, { cwd: root, encoding: 'utf8' });
     assert.equal(result.status, 0, result.stderr);
    }
   }
  }
 }
 let operation = 'check';
 if (version) operation = slowVersion ? 'slow-version' : 'version';
 return spawn(process.execPath, [fileURLToPath(new URL('continuation-child.mjs', import.meta.url)), operation], options);
};
syncBuiltinESMExports();
`;

	function continuationFixture(t) {
		const f = createRunnerFixture(t);
		writeFileSync(path.join(f.root, 'continuation-child.mjs'), continuationChild);
		const preload = path.join(f.root, 'continuation-preload.mjs');
		writeFileSync(preload, continuationPreload);
		freezeFixture(f);
		const directory = mkdtempSync(path.join(tmpdir(), 'orchbee TLC continuation '));
		t.after(() => rmSync(directory, { recursive: true, force: true }));
		return {
			...f,
			directory,
			check: (args, outcome = 'passed', extra = {}) =>
				f.invoke('run-models.mjs', [...args, '--jobs', '1', '--run-dir', directory], {
					ORCHBEE_TLA_JAVA: 'test-java',
					NODE_OPTIONS: `--import=${pathToFileURL(preload).href}`,
					TLC_TEST_OUTCOME: outcome,
					...extra,
				}),
		};
	}

	test('CLI heap ceilings reach the JVM and bind the actual heap and overridden deadline', async (t) => {
		for (const ceiling of [256, 4096, 8192]) {
			const f = continuationFixture(t);
			const result = await f.check(
				['--models', 'alpha', '--max-heap-mib', String(ceiling)],
				'passed',
				{
					TLC_TEST_MEMORY_GIB: '32',
					TLC_TEST_HEAP_MIB: String(ceiling),
					ORCHBEE_TLA_TIMEOUT_MILLISECONDS: '10800000',
				},
			);
			assert.equal(result.status, 0, result.stdout + result.stderr);
			const manifest = JSON.parse(readFileSync(path.join(f.directory, 'manifest.json'), 'utf8'));
			assert.equal(manifest.context.maximumHeapMiB, ceiling);
			assert.equal(manifest.context.heapMiB, ceiling);
			assert.ok(
				manifest.plans.every(
					(plan) => plan.heapMiB === ceiling && plan.timeoutMilliseconds === 10800000,
				),
			);
		}
	});

	test('CLI changing the heap ceiling invalidates continuation even when actual heaps match', async (t) => {
		const f = continuationFixture(t);
		const extra = { TLC_TEST_MEMORY_GIB: '4', TLC_TEST_HEAP_MIB: '1945' };
		const first = await f.check(['--models', 'alpha'], 'passed', extra);
		assert.equal(first.status, 0, first.stdout + first.stderr);
		const manifest = readFileSync(path.join(f.directory, 'manifest.json'));
		assert.equal(JSON.parse(manifest).context.maximumHeapMiB, 4096);
		const second = await f.check(['--models', 'alpha', '--max-heap-mib', '8192'], 'passed', extra);
		assert.notEqual(second.status, 0);
		assert.match(second.stderr, /TLC run identity changed; use a new --run-dir/u);
		assert.doesNotMatch(second.stdout, /^TLA_RESULT /mu);
		assert.deepEqual(readFileSync(path.join(f.directory, 'manifest.json')), manifest);
		assert.equal(readdirSync(path.join(f.directory, 'logs')).length, 1);
		assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
	});

	function runSummary(result) {
		return JSON.parse(
			result.stdout
				.split('\n')
				.find((line) => line.startsWith('TLA_RUN_RESULT '))
				.slice('TLA_RUN_RESULT '.length),
		);
	}

	for (const outcome of ['dirty-claim', 'commit-claim']) {
		test(`CLI rejects ${outcome} at completion even when TLC finishes successfully`, async (t) => {
			const f = continuationFixture(t);
			const result = await f.check(['--models', 'alpha'], outcome);
			assert.notEqual(result.status, 0, result.stdout + result.stderr);
			assert.match(result.stdout, /^TLA_RESULT /mu);
			assert.doesNotMatch(result.stdout, /Checked 1 TLA/u);
			assert.equal(runSummary(result).complete, false);
			assert.match(runSummary(result).integrityFailure, /clean frozen checkout|identity changed/u);
			assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		});
	}

	test('CLI refuses ignored claim authority even when the checkout is otherwise clean', async (t) => {
		const f = continuationFixture(t);
		const git = (...args) => {
			const result = spawnSync('git', args, { cwd: f.root, encoding: 'utf8' });
			assert.equal(result.status, 0, result.stderr);
		};
		git('rm', '--cached', 'apps/alpha/README.md');
		writeFileSync(path.join(f.root, '.gitignore'), 'apps/alpha/README.md\n');
		git('add', '.gitignore');
		git(
			'-c',
			'user.name=TLC test',
			'-c',
			'user.email=tlc@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'ignored authority',
		);
		const result = await f.check(['--models', 'alpha']);
		assert.notEqual(result.status, 0, result.stdout + result.stderr);
		assert.match(
			result.stderr,
			/Frozen TLA inputs must be tracked in HEAD.*apps\/alpha\/README\.md/u,
		);
		assert.doesNotMatch(result.stdout, /Checking alpha/u);
		assert.deepEqual(readdirSync(f.directory), []);
	});

	test('cheap input admission rejects ignored authority while permitting new development inputs', async (t) => {
		const f = createRunnerFixture(t);
		freezeFixture(f);
		const inputs = await import(pathToFileURL(path.join(f.scripts, 'model-inventory.mjs')).href);
		const newClaim = 'apps/alpha/NEW.md';
		writeFileSync(path.join(f.root, newClaim), '# New claim\n');
		await inputs.assertInputAdmission([newClaim]);
		writeFileSync(path.join(f.root, '.gitignore'), `${newClaim}\n`);
		await assert.rejects(
			inputs.assertInputAdmission([newClaim]),
			/claim authority cannot be ignored/u,
		);
	});

	for (const flag of ['--skip-worktree', '--assume-unchanged']) {
		test(`CLI rejects ${flag} inputs even when Git hides their changed claim bytes`, async (t) => {
			const f = continuationFixture(t);
			const result = spawnSync('git', ['update-index', flag, 'apps/alpha/README.md'], {
				cwd: f.root,
				encoding: 'utf8',
			});
			assert.equal(result.status, 0, result.stderr);
			writeFileSync(
				path.join(f.root, 'apps/alpha/README.md'),
				'# alpha\nUncommitted hidden claim.\n',
			);
			const status = spawnSync('git', ['status', '--porcelain'], { cwd: f.root, encoding: 'utf8' });
			assert.equal(status.stdout.trim(), '');
			const check = await f.check(['--models', 'alpha']);
			assert.notEqual(check.status, 0, check.stdout + check.stderr);
			assert.match(check.stderr, /cannot use skip-worktree or assume-unchanged/u);
			assert.doesNotMatch(check.stdout, /Checking alpha/u);
			assert.deepEqual(readdirSync(f.directory), []);
		});
	}

	test('cancellation observed after temporary record write never publishes a passed configuration', async (t) => {
		const f = continuationFixture(t);
		const result = await f.check(['--models', 'alpha'], 'cancel-config-write');
		assert.notEqual(result.status, 0, result.stdout + result.stderr);
		assert.equal(runSummary(result).cancelled, true);
		assert.ok(!existsSync(path.join(f.directory, 'records/alpha--default.json')));
		assert.deepEqual(readdirSync(path.join(f.directory, 'records')), []);
		assert.deepEqual(readdirSync(path.join(f.directory, 'inputs')), []);
		assert.deepEqual(readdirSync(path.join(f.directory, 'models')), []);
		assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		assert.doesNotMatch(result.stdout, /^TLA_RESULT /mu);
		const continued = await f.check(['--models', 'alpha']);
		assert.equal(continued.status, 0, continued.stdout + continued.stderr);
		assert.equal(runSummary(continued).freshConfigurations, 1);
		assert.equal(runSummary(continued).complete, true);
	});

	test('cancellation before aggregate publication preserves committed configurations for continuation', async (t) => {
		const f = continuationFixture(t);
		const result = await f.check(['--models', 'alpha'], 'cancel-model-write');
		assert.notEqual(result.status, 0, result.stdout + result.stderr);
		assert.equal(runSummary(result).cancelled, true);
		assert.equal(runSummary(result).passedModels, 0);
		assert.equal(runSummary(result).pendingModels, 1);
		const committed = JSON.parse(
			readFileSync(path.join(f.directory, 'records/alpha--default.json'), 'utf8'),
		);
		assert.equal(committed.status, 'passed');
		assert.deepEqual(readdirSync(path.join(f.directory, 'models')), []);
		assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		const continued = await f.check(['--models', 'alpha']);
		assert.equal(continued.status, 0, continued.stdout + continued.stderr);
		assert.equal(runSummary(continued).freshConfigurations, 0);
		assert.equal(runSummary(continued).reusedConfigurations, 1);
		assert.equal(runSummary(continued).complete, true);
	});

	test('CLI cancellation during lease acquisition releases the lock before reusing committed evidence', async (t) => {
		const f = continuationFixture(t);
		assert.equal((await f.check(['--models', 'alpha'])).status, 0);
		const result = await f.check(['--models', 'alpha'], 'cancel-acquire');
		assert.notEqual(result.status, 0, result.stdout + result.stderr);
		assert.match(result.stderr, /TLC checking cancelled/u);
		assert.equal(runSummary(result).complete, false);
		assert.equal(runSummary(result).cancelled, true);
		assert.equal(runSummary(result).freshConfigurations, 0);
		assert.equal(runSummary(result).reusedConfigurations, 0);
		assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		assert.doesNotMatch(result.stdout, /^TLA_RESULT /mu);
	});

	for (const outcome of ['cancel-final-probe', 'cancel-final-close']) {
		test(`CLI ${outcome} fails the invocation and releases its lease after valid TLC completion`, async (t) => {
			const f = continuationFixture(t);
			const result = await f.check(['--models', 'alpha'], outcome);
			assert.notEqual(result.status, 0, result.stdout + result.stderr);
			assert.match(result.stdout, /^TLA_RESULT /mu);
			assert.match(result.stderr, /TLC checking cancelled/u);
			assert.equal(runSummary(result).complete, false);
			assert.equal(runSummary(result).cancelled, true);
			assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
			assert.deepEqual(readdirSync(path.join(f.directory, 'attempts')), []);
			assert.doesNotMatch(result.stdout, /Checked 1 TLA/u);
		});
	}

	test('CLI continuation reuses exact completed evidence and expands only its requested scope', async (t) => {
		const f = continuationFixture(t);
		const first = await f.check(['--models', 'alpha']);
		assert.equal(first.status, 0, first.stdout + first.stderr);
		assert.equal(runSummary(first).freshConfigurations, 1);
		const continued = await f.check(['--all']);
		assert.equal(continued.status, 0, continued.stdout + continued.stderr);
		assert.equal(runSummary(continued).freshConfigurations, 1);
		assert.equal(runSummary(continued).reusedConfigurations, 1);
		assert.equal(runSummary(continued).passedModels, 2);
		const reports = continued.stdout
			.split('\n')
			.filter((line) => line.startsWith('TLA_RESULT '))
			.map((line) => JSON.parse(line.slice('TLA_RESULT '.length)));
		const original = JSON.parse(
			first.stdout
				.split('\n')
				.find((line) => line.startsWith('TLA_RESULT '))
				.slice('TLA_RESULT '.length),
		);
		assert.equal(reports[0].durationMilliseconds, original.durationMilliseconds);
		assert.equal(reports[0].modelConfigSha256, original.modelConfigSha256);
		assert.equal(reports[0].reused, true);
		assert.deepEqual(readdirSync(path.join(f.directory, 'attempts')), []);
		assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		assert.equal(readdirSync(path.join(f.directory, 'logs')).length, 2);
	});

	test('CLI historical failure remains failed while unchecked work proceeds on the next invocation', async (t) => {
		const f = continuationFixture(t);
		const first = await f.check(['--all'], 'failed');
		assert.notEqual(first.status, 0);
		assert.equal(runSummary(first).freshConfigurations, 1);
		const continued = await f.check(['--all']);
		assert.notEqual(continued.status, 0, continued.stdout + continued.stderr);
		assert.equal(runSummary(continued).freshConfigurations, 1);
		assert.equal(runSummary(continued).failedModels, 1);
		assert.equal(runSummary(continued).passedModels, 1);
		assert.equal(runSummary(continued).complete, false);
		const selected = await f.check(['--models', 'beta']);
		assert.equal(selected.status, 0, selected.stdout + selected.stderr);
		assert.equal(runSummary(selected).complete, false);
		assert.equal(runSummary(selected).freshConfigurations, 0);
		assert.equal(readdirSync(path.join(f.directory, 'logs')).length, 2);
	});

	test('CLI rejects dirty identity and corrupted committed logs before launching TLC', async (t) => {
		const f = continuationFixture(t);
		assert.equal((await f.check(['--models', 'alpha'])).status, 0);
		const original = readFileSync(path.join(f.root, f.inventory.models[0].config));
		writeFileSync(path.join(f.root, f.inventory.models[0].config), 'changed');
		const dirty = await f.check(['--all']);
		assert.notEqual(dirty.status, 0);
		assert.match(dirty.stderr, /clean frozen checkout/u);
		assert.doesNotMatch(dirty.stdout, /Checking alpha/u);
		writeFileSync(path.join(f.root, f.inventory.models[0].config), original);
		const record = JSON.parse(
			readFileSync(path.join(f.directory, 'records/alpha--default.json'), 'utf8'),
		);
		writeFileSync(path.join(f.directory, record.log), 'corrupt');
		const corrupt = await f.check(['--all']);
		assert.notEqual(corrupt.status, 0);
		assert.match(corrupt.stderr, /Corrupt TLC log/u);
		assert.doesNotMatch(corrupt.stdout, /Checking beta/u);
		assert.equal(readdirSync(path.join(f.directory, 'logs')).length, 1);
	});

	for (const outcome of ['incomplete', 'cancel']) {
		test(`CLI ${outcome} evidence never commits a reusable pass and releases owned attempts`, async (t) => {
			const f = continuationFixture(t);
			const result = await f.check(['--all'], outcome);
			assert.notEqual(result.status, 0, result.stdout + result.stderr);
			assert.doesNotMatch(result.stdout, /^TLA_RESULT /mu);
			assert.equal(runSummary(result).failedModels, 1);
			assert.equal(runSummary(result).pendingModels, 1);
			assert.deepEqual(readdirSync(path.join(f.directory, 'attempts')), []);
			assert.ok(!existsSync(path.join(f.directory, '.tla-run.lock')));
		});
	}
});
