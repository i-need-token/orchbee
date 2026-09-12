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
import test from 'node:test';
import { pathToFileURL } from 'node:url';

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

test('concurrent variants retain private modules after a sibling exits and clean both directories', () => {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee TLC runner isolation '));
	const scripts = path.join(root, 'scripts/tla');
	const formal = path.join(root, 'scripts/formal/tla');
	const tools = path.join(root, '.tools');
	try {
		mkdirSync(scripts, { recursive: true });
		mkdirSync(formal, { recursive: true });
		mkdirSync(tools);
		for (const filename of ['run-models.mjs', 'model-checks.mjs', 'model-inventory.mjs']) {
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
		const result = spawnSync(
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
		'model-inventory.mjs',
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
		spawnSync(process.execPath, [path.join(scripts, entry), ...args], {
			cwd: root,
			env: { ...env, ...extra },
			encoding: 'utf8',
			timeout: 20_000,
		});
	return { root, scripts, tools, inventory, inventoryFile, invoke };
}

test('CLI reads the requested Git inventory base and fails safe for missing or malformed bases', (t) => {
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
	fixture.inventory.models[0].triggers.push('apps/moved/src/**');
	writeFileSync(fixture.inventoryFile, JSON.stringify(fixture.inventory));
	const scoped = fixture.invoke('select-changed-models.mjs', [base]);
	assert.equal(scoped.status, 0, scoped.stderr);
	assert.equal(scoped.stdout, 'alpha\n');
	const unknown = fixture.invoke('select-changed-models.mjs', ['missing-reference']);
	assert.equal(unknown.status, 0);
	assert.equal(unknown.stdout, 'alpha,beta\n');
	assert.match(unknown.stderr, /selecting all/u);
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
	const fallback = fixture.invoke('select-changed-models.mjs', [malformed]);
	assert.equal(fallback.status, 0);
	assert.equal(fallback.stdout, 'alpha,beta\n');
	assert.match(fallback.stderr, /selecting all/u);
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
	test(`runner records ${outcome}, retains its log, and identifies unstarted work without claiming success`, (t) => {
		const fixture = createRunnerFixture(t);
		writeFileSync(path.join(fixture.root, 'outcome-child.mjs'), outcomeChild);
		const preload = path.join(fixture.root, 'outcome-preload.mjs');
		writeFileSync(preload, outcomePreload);
		const result = fixture.invoke('run-models.mjs', ['--all', '--jobs', '1'], {
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
