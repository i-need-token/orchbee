import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
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
