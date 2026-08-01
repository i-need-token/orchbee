import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const toolsDirectory = path.join(packageRoot, '.tools');
const tlaVersion = '1.7.4';
const expectedSha1 = 'bee4a54f3ee3d4afc347c3240ec2d9e93b075104';
const bundledJarPath = path.join(toolsDirectory, `tla2tools-${tlaVersion}.jar`);
const configuredJarPath = process.env.TERMINAL_PROTOCOL_TLA2TOOLS_JAR;
const jarPath = configuredJarPath ? path.resolve(configuredJarPath) : bundledJarPath;
const models = ['TerminalHostLifecycle', 'TerminalFrameBackpressure'];

async function sha1(filePath) {
	const contents = await readFile(filePath);
	return createHash('sha1').update(contents).digest('hex');
}

async function jarIsValid(filePath) {
	try {
		return (await sha1(filePath)) === expectedSha1;
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			return false;
		}
		throw error;
	}
}

async function provisionJar() {
	if (await jarIsValid(jarPath)) return;
	if (configuredJarPath) {
		throw new Error(
			`TERMINAL_PROTOCOL_TLA2TOOLS_JAR does not match pinned TLA+ ${tlaVersion} SHA-1`,
		);
	}
	await mkdir(toolsDirectory, { recursive: true });
	const downloadUrl = `https://github.com/tlaplus/tlaplus/releases/download/v${tlaVersion}/tla2tools.jar`;
	const response = await fetch(downloadUrl);
	if (!response.ok) throw new Error(`Could not download ${downloadUrl}: HTTP ${response.status}`);
	const temporaryPath = `${jarPath}.download`;
	await writeFile(temporaryPath, new Uint8Array(await response.arrayBuffer()));
	if (!(await jarIsValid(temporaryPath))) {
		await rm(temporaryPath, { force: true });
		throw new Error(`Downloaded TLA+ ${tlaVersion} jar failed its pinned SHA-1 check`);
	}
	await rename(temporaryPath, jarPath);
}

async function runModel(model) {
	const modelPath = path.join(packageRoot, 'formal', 'tla', `${model}.tla`);
	const configPath = path.join(packageRoot, 'formal', 'tla', `${model}.cfg`);
	const metadataDirectory = await mkdtemp(path.join(tmpdir(), `orchbee-tla-${model}-`));
	try {
		const child = spawn(
			'java',
			[
				'-XX:+UseParallelGC',
				'-jar',
				jarPath,
				'-fp',
				'0',
				'-workers',
				'1',
				'-metadir',
				metadataDirectory,
				'-config',
				configPath,
				modelPath,
			],
			{ cwd: packageRoot, stdio: 'inherit' },
		);
		await new Promise((resolve, reject) => {
			child.once('error', reject);
			child.once('exit', (code, signal) => {
				if (signal) {
					reject(new Error(`${model} TLC terminated by ${signal}`));
					return;
				}
				if (code !== 0) {
					reject(new Error(`${model} TLC exited with status ${code}`));
					return;
				}
				resolve();
			});
		});
	} finally {
		await rm(metadataDirectory, { force: true, recursive: true });
	}
}

await provisionJar();
await models.reduce(
	(previous, model) => previous.then(async () => runModel(model)),
	Promise.resolve(),
);
