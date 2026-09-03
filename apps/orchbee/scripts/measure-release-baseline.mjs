import { execFileSync, spawnSync } from 'node:child_process';
import { statSync } from 'node:fs';
import { arch, platform, release } from 'node:os';
import { resolve } from 'node:path';
import { performance } from 'node:perf_hooks';

const WARMUP_INVOCATIONS = 5;
const MEASURED_INVOCATIONS = 50;
const repositoryRoot = resolve(import.meta.dirname, '..', '..', '..');
const binaryName = platform() === 'win32' ? 'orchbee.exe' : 'orchbee';
const binaryPath = resolve(repositoryRoot, 'target', 'release', binaryName);

execFileSync('cargo', ['build', '-p', 'orchbee', '--release', '--locked'], {
	cwd: repositoryRoot,
	stdio: 'inherit',
});

for (let index = 0; index < WARMUP_INVOCATIONS; index += 1) invokeVersion();

const durations = [];
for (let index = 0; index < MEASURED_INVOCATIONS; index += 1) {
	const start = performance.now();
	invokeVersion();
	durations.push(performance.now() - start);
}
durations.sort((left, right) => left - right);

const report = {
	architecture: arch(),
	artifactBytes: statSync(binaryPath).size,
	command: 'orchbee --version',
	operatingSystem: platform(),
	operatingSystemRelease: release(),
	peakResidentBytes: measurePeakResidentBytes(),
	profile: 'release',
	rustc: execFileSync('rustc', ['--version'], { cwd: repositoryRoot, encoding: 'utf8' }).trim(),
	schemaVersion: 1,
	startupMilliseconds: {
		maximum: round(durations.at(-1)),
		minimum: round(durations[0]),
		p50: round(percentile(durations, 0.5)),
		p95: round(percentile(durations, 0.95)),
	},
	warmupInvocations: WARMUP_INVOCATIONS,
	measuredInvocations: MEASURED_INVOCATIONS,
};

process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);

function invokeVersion() {
	const result = spawnSync(binaryPath, ['--version'], { encoding: 'utf8' });
	if (
		result.error !== undefined ||
		result.status !== 0 ||
		result.stderr !== '' ||
		!/^orchbee [^\r\n]+\n$/u.test(result.stdout)
	) {
		throw new Error(
			`Measured invocation failed: ${result.error?.message ?? `status ${result.status}`}`,
		);
	}
}

function measurePeakResidentBytes() {
	if (platform() === 'darwin') {
		const result = spawnSync('/usr/bin/time', ['-l', binaryPath, '--version'], {
			encoding: 'utf8',
		});
		if (result.error !== undefined || result.status !== 0) {
			throw new Error(`Peak-memory invocation failed: ${result.error?.message ?? result.stderr}`);
		}
		const match = result.stderr.match(/^\s*(\d+)\s+maximum resident set size$/mu);
		if (match?.[1] === undefined) {
			throw new Error('macOS /usr/bin/time did not report maximum resident set size');
		}
		return Number.parseInt(match[1], 10);
	}

	if (platform() === 'linux') {
		const result = spawnSync('/usr/bin/time', ['-v', binaryPath, '--version'], {
			encoding: 'utf8',
		});
		if (result.error !== undefined || result.status !== 0) {
			throw new Error(`Peak-memory invocation failed: ${result.error?.message ?? result.stderr}`);
		}
		const match = result.stderr.match(/^\s*Maximum resident set size \(kbytes\):\s*(\d+)$/mu);
		if (match?.[1] === undefined) {
			throw new Error('GNU /usr/bin/time did not report maximum resident set size');
		}
		return Number.parseInt(match[1], 10) * 1024;
	}

	return null;
}

function percentile(sortedValues, fraction) {
	const index = Math.max(0, Math.ceil(sortedValues.length * fraction) - 1);
	const value = sortedValues[index];
	if (value === undefined) throw new Error('Cannot select a percentile from an empty sample');
	return value;
}

function round(value) {
	if (value === undefined) throw new Error('Measurement sample was unexpectedly empty');
	return Number(value.toFixed(3));
}
