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
const configuredJarPath = process.env.TERMINAL_TLA2TOOLS_JAR;
const jarPath = configuredJarPath ? path.resolve(configuredJarPath) : bundledJarPath;

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
		throw new Error(`TERMINAL_TLA2TOOLS_JAR does not match pinned TLA+ ${tlaVersion} SHA-1`);
	}
	await mkdir(toolsDirectory, { recursive: true });
	const downloadUrl = `https://github.com/tlaplus/tlaplus/releases/download/v${tlaVersion}/tla2tools.jar`;
	const response = await fetch(downloadUrl);
	if (!response.ok) {
		throw new Error(`Could not download ${downloadUrl}: HTTP ${response.status}`);
	}
	const temporaryPath = `${jarPath}.download`;
	await writeFile(temporaryPath, new Uint8Array(await response.arrayBuffer()));
	if (!(await jarIsValid(temporaryPath))) {
		await rm(temporaryPath, { force: true });
		throw new Error(`Downloaded TLA+ ${tlaVersion} jar failed its pinned SHA-1 check`);
	}
	await rename(temporaryPath, jarPath);
}

const modelNames = ['TerminalPresentationConfiguration', 'TerminalInputRepair'];
const modelScenarios = new Map([
	[
		'TerminalInputRepair',
		[
			'composition',
			'key',
			'mouse',
			'focus',
			'review-key',
			'review-mouse',
			'review-focus',
			'review-composition',
			'lifecycle',
		],
	],
]);
const requiredCoverage = new Map([
	[
		'TerminalInputRepair',
		[
			'BeginPasteReview',
			'ResolvePasteReview',
			'RejectPasteReview',
			'ResolveCancelledReview',
			'QueueKeyPress',
			'QueueKeyRelease',
			'QueueMousePress',
			'QueueMouseRelease',
			'QueueFocusIn',
			'PublishQueuedKeyPress',
			'PublishQueuedKeyRelease',
			'PublishQueuedMousePress',
			'PublishQueuedMouseRelease',
			'PublishQueuedFocusIn',
			'CoverageCompositionAuthorityAntecedent',
			'CoverageExplicitPublicationOriginAntecedent',
			'CoverageEligibleRepairGuardAntecedent',
			'CoverageSettledRepairGuardAntecedent',
			'CoverageFocusOutOwnershipAntecedent',
			'CoverageUnfocusedDisposeAntecedent',
			'CoverageDisposeAfterFocusRepairAntecedent',
			'CoveragePendingDisposeRequestAntecedent',
			'CoveragePendingDisposeBeginAntecedent',
			'CoverageFocusedActiveResetState',
			'CoverageStartCoalescedResetFocusRepair',
			'CoverageCoalesceFocusRepairDuringActiveReset',
			'CoverageStagedInvalidationAntecedent',
			'CoverageCurrentContinuationAntecedent',
			'CoverageStaleDiscardAntecedent',
			'CoverageStalledReviewState',
			'CoverageQueuedKeyPressMismatch',
			'CoverageQueuedKeyReleaseMismatch',
			'CoverageQueuedMousePressMismatch',
			'CoverageQueuedMouseReleaseMismatch',
			'CoverageQueuedFocusMismatch',
			'CoveragePublishedKeyCleanupAntecedent',
			'CoverageSkippedKeyCleanupAntecedent',
			'CoveragePublishedMouseCleanupAntecedent',
			'CoverageSkippedMouseCleanupAntecedent',
			'CoverageCancelledReviewResolution',
			'CoverageCancelledReviewBypassState',
			'CoverageStatelessSendSuccess',
			'CoverageSendFailure',
			'CoverageFailedKeyPressRollback',
			'CoverageFailedMousePressRollback',
			'CoveragePublishedKeyReleaseAntecedent',
			'CoveragePublishedMouseReleaseAntecedent',
			'CoverageReentrantRepairDuringSend',
			'CoverageFocusReconciliation',
			'CoverageRepairReopenFocusReconciliation',
			'CoveragePasteReviewRejection',
			'CoverageRepairSendFailure',
			'CoverageCurrentContinuationEffect',
			'CoverageStaleContinuationEffect',
			'BlockTextPublicationOnFocusMismatch',
			'CoverageReviewCompositionResetFocusFailure',
			'CoverageLocalCompositionProgressWithFocusMismatch',
			'CoverageLocalSettlementWithFocusMismatch',
			'CoveragePostConvergenceCompositionPublication',
		],
	],
]);

function escapeRegularExpression(value) {
	return value.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function assertRequiredCoverage(modelName, output, markers) {
	for (const marker of markers) {
		const expression = new RegExp(
			`^<${escapeRegularExpression(marker)}\\b[^\\n]*>:\\s*([\\d,]+)(?::([\\d,]+))?\\s*$`,
			'gm',
		);
		const matches = [...output.matchAll(expression)];
		if (matches.length === 0) {
			throw new Error(`${modelName} TLC coverage is missing required marker ${marker}`);
		}
		const counts = matches.flatMap((match) =>
			match
				.slice(1)
				.filter((value) => value !== undefined)
				.map((value) => Number.parseInt(value.replaceAll(',', ''), 10)),
		);
		if (!counts.some((count) => Number.isFinite(count) && count > 0)) {
			throw new Error(`${modelName} TLC coverage marker ${marker} was not reached`);
		}
	}
}

function verifyCoverageEnforcement() {
	const marker = 'CoverageSelfCheck';
	assertRequiredCoverage('self-check', `<${marker} line 1>: 1:1`, [marker]);
	assertRequiredCoverage('self-check', `<${marker} line 1>: 0:1`, [marker]);
	for (const output of [`<${marker} line 1>: 0:0`, '<DifferentMarker line 1>: 1:1']) {
		let rejected = false;
		try {
			assertRequiredCoverage('self-check', output, [marker]);
		} catch {
			rejected = true;
		}
		if (!rejected) throw new Error('TLC coverage enforcement self-check accepted missing coverage');
	}
}

async function runTlc(modelName, scenario) {
	const modelPath = path.join(packageRoot, 'formal', 'tla', `${modelName}.tla`);
	const configPath = path.join(packageRoot, 'formal', 'tla', `${modelName}.cfg`);
	const metadataDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-tla-'));
	let executionConfigPath = configPath;
	try {
		let executionConfigFileName = path.basename(configPath);
		if (scenario !== undefined) {
			const config = await readFile(configPath, 'utf8');
			executionConfigFileName = `${modelName}-${scenario}.cfg`;
			executionConfigPath = path.join(packageRoot, 'formal', 'tla', executionConfigFileName);
			await writeFile(
				executionConfigPath,
				config.replace('CONSTANT Scenario = "composition"', `CONSTANT Scenario = "${scenario}"`),
			);
			process.stdout.write(`Checking ${modelName} scenario ${scenario}\n`);
		}
		const child = spawn(
			'java',
			[
				'-XX:+UseParallelGC',
				'-jar',
				jarPath,
				'-metadir',
				metadataDirectory,
				'-fp',
				'0',
				'-coverage',
				'1',
				'-workers',
				'1',
				'-config',
				executionConfigFileName,
				modelPath,
			],
			{
				cwd: scenario === undefined ? packageRoot : path.dirname(configPath),
				stdio: ['ignore', 'pipe', 'pipe'],
			},
		);
		let output = '';
		child.stdout.on('data', (chunk) => {
			output += chunk.toString();
			process.stdout.write(chunk);
		});
		child.stderr.on('data', (chunk) => {
			process.stderr.write(chunk);
		});
		await new Promise((resolve, reject) => {
			let settled = false;
			child.once('error', (error) => {
				if (settled) return;
				settled = true;
				reject(error);
			});
			child.once('exit', (code, signal) => {
				if (settled) return;
				settled = true;
				if (signal) {
					reject(new Error(`TLC terminated by ${signal}`));
					return;
				}
				if (code !== 0) {
					reject(new Error(`TLC exited with status ${code}`));
					return;
				}
				resolve();
			});
		});
		return output;
	} finally {
		if (scenario !== undefined) await rm(executionConfigPath, { force: true });
		await rm(metadataDirectory, { force: true, recursive: true });
	}
}

verifyCoverageEnforcement();
await provisionJar();
for (const modelName of modelNames) {
	let output = '';
	for (const scenario of modelScenarios.get(modelName) ?? [undefined]) {
		output += await runTlc(modelName, scenario);
	}
	assertRequiredCoverage(modelName, output, requiredCoverage.get(modelName) ?? []);
}
