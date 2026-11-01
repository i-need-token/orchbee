import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, mkdtempSync, openSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// This private gate owns acceptance, not Knip's raw result or package retention decisions.
const knipVersion = '6.26.0';
const analysisInputs = [
	{
		file: 'knip.config.mjs',
		sha256: 'eebd43b0ba5e0544e1ba52b73401313ce07444b408b0687b15c2c6aa1bd7db53',
	},
	{
		file: 'pnpm-workspace.yaml',
		sha256: 'af2b7c011fe2125dad1e65290ff2d5fa8d1c39f61f5466795a2761b6b1374a61',
	},
	{
		file: 'tsconfig.base.json',
		sha256: 'b6cbd112708bf72eafa3ff455dfc5581bfe467d29a791d6d34a028c20cfaf59d',
	},
	{
		file: '.gitignore',
		sha256: '8385428233d4a4029dc255fccf125060d7d004f46f8b0dc00cc3cb0dc945b1ec',
	},
];
const retentionOwners = [
	['packages/ui/dependency-retentions.json', '@orchbee/ui'],
	['packages/tab-strip/dependency-retentions.json', '@orchbee/tab-strip'],
];
const issueKinds = [
	'files',
	'dependencies',
	'devDependencies',
	'optionalPeerDependencies',
	'unlisted',
	'binaries',
	'unresolved',
	'exports',
	'nsExports',
	'types',
	'nsTypes',
	'enumMembers',
	'namespaceMembers',
	'duplicates',
	'catalog',
	'cycles',
];
const marker = '@@orchbee-knip-evidence@@';
const modulePath = fileURLToPath(import.meta.url);

function requireCondition(condition, message) {
	if (!condition) throw new Error(`${message}; see docs/LINTING.md#retained-development-consumers`);
}

function objectKeys(value, required, optional = []) {
	requireCondition(
		value !== null && typeof value === 'object' && !Array.isArray(value),
		'Expected an object',
	);
	requireCondition(
		required.every((key) => Object.hasOwn(value, key)),
		'Missing required field',
	);
	requireCondition(
		Object.keys(value).every((key) => [...required, ...optional].includes(key)),
		'Unknown field',
	);
}

function textValue(value) {
	requireCondition(
		typeof value === 'string' &&
			value.length > 0 &&
			[...value].every((character) => character.codePointAt(0) >= 32),
		'Invalid text',
	);
}

function repositoryPath(value) {
	textValue(value);
	requireCondition(
		!/[\\:*?[\]{}]/u.test(value) &&
			value.split('/').every((part) => part !== '' && part !== '.' && part !== '..'),
		`Noncanonical path: ${value}`,
	);
}

function digest(value) {
	return createHash('sha256').update(value).digest('hex');
}

function tuple(row) {
	return JSON.stringify([row.kind, row.file, row.name]);
}

function validateTuple(row) {
	objectKeys(row, ['kind', 'file', 'name']);
	requireCondition(
		['files', 'exports', 'types'].includes(row.kind),
		'Retention cannot admit this issue category',
	);
	repositoryPath(row.file);
	textValue(row.name);
	if (row.kind === 'files')
		requireCondition(row.name === row.file, 'File diagnostic name must equal its path');
}

// Full source fingerprints bind the reviewed JSX/calls/assertions and their discovery/resolve inputs.
// There is deliberately no refresh command. Changing a digest requires renewed semantic review.
export function readRetentionPolicy(
	root,
	readText = (file) => readFileSync(join(root, file), 'utf8'),
) {
	const observed = [];
	const inspect = (file) => {
		const contents = readText(file);
		requireCondition(typeof contents === 'string', `Missing input: ${file}`);
		observed.push([file, digest(contents)]);
		return contents;
	};
	const fingerprint = ({ file, sha256 }) => {
		repositoryPath(file);
		requireCondition(
			typeof sha256 === 'string' && /^[a-f0-9]{64}$/u.test(sha256),
			`Invalid fingerprint: ${file}`,
		);
		requireCondition(
			digest(inspect(file)) === sha256,
			`Reviewed consumer/configuration changed: ${file}`,
		);
	};
	for (const input of analysisInputs) fingerprint(input);
	const installed = JSON.parse(inspect('node_modules/knip/package.json'));
	requireCondition(
		installed.version === knipVersion,
		'Knip reporter contract needs review after a version change',
	);
	const rootManifest = JSON.parse(inspect('package.json'));
	requireCondition(
		!Object.hasOwn(rootManifest, 'knip'),
		'Undeclared package.json Knip configuration',
	);
	requireCondition(
		rootManifest.scripts?.['lint:dependencies'] === 'node scripts/dependency-retentions.mjs',
		'Dependency gate entry changed',
	);
	const expected = [];
	const identities = new Set();
	for (const [file, owner] of retentionOwners) {
		const manifest = JSON.parse(inspect(file));
		objectKeys(manifest, ['version', 'owner', 'records']);
		requireCondition(
			manifest.version === 1 && manifest.owner === owner,
			`Invalid retention owner/version: ${file}`,
		);
		requireCondition(
			Array.isArray(manifest.records) && manifest.records.length > 0,
			`Remove an exhausted retention manifest and its registration: ${file}`,
		);
		const workspace = dirname(file).replaceAll('\\', '/');
		const recordIds = new Set();
		for (const record of manifest.records) {
			objectKeys(record, [
				'id',
				'reason',
				'followUp',
				'removeWhen',
				'consumers',
				'inputs',
				'diagnostics',
			]);
			for (const field of ['id', 'reason', 'followUp', 'removeWhen']) textValue(record[field]);
			requireCondition(!recordIds.has(record.id), `Duplicate retention record: ${record.id}`);
			recordIds.add(record.id);
			for (const field of ['consumers', 'inputs', 'diagnostics'])
				requireCondition(
					Array.isArray(record[field]) && record[field].length > 0,
					`Empty retention ${field}`,
				);
			const inputPaths = new Set();
			for (const input of record.inputs) {
				objectKeys(input, ['file', 'sha256']);
				requireCondition(!inputPaths.has(input.file), `Duplicate evidence input: ${input.file}`);
				inputPaths.add(input.file);
				fingerprint(input);
			}
			const consumers = new Set();
			for (const consumer of record.consumers) {
				objectKeys(consumer, ['file', 'use']);
				repositoryPath(consumer.file);
				textValue(consumer.use);
				requireCondition(
					consumer.file.startsWith(`${workspace}/`) && inputPaths.has(consumer.file),
					`Consumer lacks owner-scoped reviewed source: ${consumer.file}`,
				);
				requireCondition(!consumers.has(consumer.file), `Duplicate consumer: ${consumer.file}`);
				consumers.add(consumer.file);
			}
			for (const row of record.diagnostics) {
				validateTuple(row);
				requireCondition(
					row.file.startsWith(`${workspace}/`) && inputPaths.has(row.file),
					`Retention lacks its owned source evidence: ${row.file}`,
				);
				requireCondition(
					!identities.has(tuple(row)),
					`Duplicate retained diagnostic: ${tuple(row)}`,
				);
				identities.add(tuple(row));
				expected.push(row);
			}
		}
	}
	return { expected, fingerprint: digest(JSON.stringify(observed)) };
}

// Knip's built-in JSON reporter omits hints. This second reporter adds evidence only;
// it does not mutate issues, counters, rules, or process.exitCode.
export function reportKnipEvidence(data) {
	requireCondition(
		data.tagHints instanceof Set && Array.isArray(data.configurationHints),
		'Unsupported Knip hint contract',
	);
	const evidence = {
		version: 1,
		configFile: relative(data.cwd, data.configFilePath).replaceAll('\\', '/'),
		production: data.isProduction,
		configHintsDisabled: data.isDisableConfigHints,
		configHintsAsErrors: data.isTreatConfigHintsAsErrors,
		tagHintsDisabled: data.isDisableTagHints,
		tagHintsAsErrors: data.isTreatTagHintsAsErrors,
		report: data.report,
		counters: data.counters,
		configurationHints: data.configurationHints,
		tagHints: [...data.tagHints],
	};
	process.stdout.write(`${marker}${JSON.stringify(evidence)}\n`);
}

function parseRun(run, production) {
	objectKeys(run, ['status', 'signal', 'error', 'stdout', 'stderr']);
	requireCondition(
		run.error === null && run.signal === null && [0, 1].includes(run.status),
		'Knip process failed, was interrupted, or timed out',
	);
	requireCondition(
		typeof run.stdout === 'string' && run.stderr === '',
		'Unexpected or unreadable Knip output; inspect raw logs',
	);
	const lines = run.stdout.trim().split(/\r?\n/u);
	// This exact, bounded diagnostic comes from the current Workbench Vitest configuration.
	if (
		/^\[workbench-vitest\] availableParallelism=[1-9]\d* configuredMaxWorkers=[1-9]\d*$/u.test(
			lines[0],
		)
	)
		lines.shift();
	requireCondition(
		lines.length === 2 && lines[1].startsWith(marker),
		'Expected one JSON report and one evidence envelope',
	);
	const report = JSON.parse(lines[0]);
	const evidence = JSON.parse(lines[1].slice(marker.length));
	objectKeys(report, ['issues']);
	objectKeys(evidence, [
		'version',
		'configFile',
		'production',
		'configHintsDisabled',
		'configHintsAsErrors',
		'tagHintsDisabled',
		'tagHintsAsErrors',
		'report',
		'counters',
		'configurationHints',
		'tagHints',
	]);
	requireCondition(
		evidence.version === 1 &&
			evidence.configFile === 'knip.config.mjs' &&
			evidence.production === production,
		'Unexpected Knip analysis mode/configuration',
	);
	// Knip 6 disables configuration hints in production mode itself. Full mode must check them.
	requireCondition(
		evidence.configHintsDisabled === production &&
			evidence.configHintsAsErrors === true &&
			evidence.tagHintsDisabled === false &&
			evidence.tagHintsAsErrors === false,
		'Knip checking options changed',
	);
	for (const field of ['configurationHints', 'tagHints'])
		requireCondition(
			Array.isArray(evidence[field]) && evidence[field].length === 0,
			`Knip ${field} require review`,
		);
	objectKeys(evidence.report, issueKinds);
	objectKeys(evidence.counters, [...issueKinds, 'processed', 'total']);
	for (const count of Object.values(evidence.counters))
		requireCondition(Number.isSafeInteger(count) && count >= 0, 'Invalid Knip counter');
	const enabled = issueKinds.filter(
		(kind) =>
			![
				'nsExports',
				'nsTypes',
				'cycles',
				...(production ? ['devDependencies', 'catalog'] : []),
			].includes(kind),
	);
	for (const kind of issueKinds)
		requireCondition(
			evidence.report[kind] === enabled.includes(kind),
			`Knip report category changed: ${kind}`,
		);
	requireCondition(Array.isArray(report.issues), 'Invalid Knip issues');
	const rows = [];
	const files = new Set();
	for (const issue of report.issues) {
		objectKeys(issue, ['file', ...enabled]);
		repositoryPath(issue.file);
		requireCondition(!files.has(issue.file), `Duplicate issue file: ${issue.file}`);
		files.add(issue.file);
		for (const kind of enabled) {
			requireCondition(Array.isArray(issue[kind]), `Invalid issue array: ${kind}`);
			if (!['files', 'exports', 'types'].includes(kind))
				requireCondition(
					issue[kind].length === 0,
					`Unretainable dependency/boundary diagnostic: ${kind} / ${issue.file}`,
				);
			for (const item of issue[kind]) {
				objectKeys(item, ['name'], kind === 'files' ? [] : ['line', 'col', 'pos']);
				for (const position of ['line', 'col', 'pos'])
					if (Object.hasOwn(item, position))
						requireCondition(
							Number.isSafeInteger(item[position]) &&
								item[position] >= (position === 'pos' ? 0 : 1),
							'Invalid diagnostic position',
						);
				const row = { kind, file: issue.file, name: item.name };
				validateTuple(row);
				rows.push(row);
			}
		}
	}
	const identities = rows.map(tuple);
	requireCondition(new Set(identities).size === identities.length, 'Duplicate Knip diagnostic');
	for (const kind of issueKinds)
		requireCondition(
			evidence.counters[kind] === rows.filter((row) => row.kind === kind).length,
			`Report/counter mismatch: ${kind}`,
		);
	requireCondition(
		run.status === (rows.length === 0 ? 0 : 1),
		'Raw Knip exit disagrees with its report',
	);
	return identities;
}

export function assessDependencyReports(full, strict, expected) {
	requireCondition(full.status === 0, 'Full Knip must exit zero');
	requireCondition(parseRun(full, false).length === 0, 'Full Knip must have no diagnostics');
	const actual = new Set(parseRun(strict, true));
	requireCondition(Array.isArray(expected), 'Invalid retained diagnostic collection');
	for (const row of expected) validateTuple(row);
	const approved = new Set(expected.map(tuple));
	requireCondition(approved.size === expected.length, 'Duplicate retained diagnostic');
	const extra = [...actual].filter((key) => !approved.has(key));
	const stale = [...approved].filter((key) => !actual.has(key));
	requireCondition(extra.length === 0, `Unregistered strict diagnostics: ${extra.join(', ')}`);
	requireCondition(stale.length === 0, `Stale retention records: ${stale.join(', ')}`);
	return {
		accepted: true,
		rawFullExit: full.status,
		rawStrictExit: strict.status,
		retained: expected,
	};
}

function captureKnip(root, logRoot, mode) {
	const stdoutPath = join(logRoot, `${mode}.stdout.log`);
	const stderrPath = join(logRoot, `${mode}.stderr.log`);
	const argv = [
		join(root, 'node_modules/knip/bin/knip.js'),
		'--config',
		'knip.config.mjs',
		...(mode === 'strict' ? ['--strict'] : []),
		'--reporter',
		'json',
		'--reporter',
		join(dirname(modulePath), 'dependency-retentions-reporter.config.mjs'),
	];
	const startedUtc = new Date().toISOString();
	const stdoutFd = openSync(stdoutPath, 'wx', 0o600);
	let stderrFd;
	let child;
	try {
		stderrFd = openSync(stderrPath, 'wx', 0o600);
		child = spawnSync(process.execPath, argv, {
			cwd: root,
			stdio: ['ignore', stdoutFd, stderrFd],
			timeout: 180_000,
			killSignal: 'SIGKILL',
			windowsHide: true,
		});
	} finally {
		closeSync(stdoutFd);
		if (stderrFd !== undefined) closeSync(stderrFd);
	}
	const raw = {
		status: child.status,
		signal: child.signal,
		error: child.error ? String(child.error) : null,
	};
	writeFileSync(
		join(logRoot, `${mode}.result.json`),
		`${JSON.stringify({ command: [process.execPath, ...argv], cwd: root, startedUtc, endedUtc: new Date().toISOString(), ...raw, stdoutPath, stderrPath }, null, 2)}\n`,
	);
	process.stdout.write(
		`[dependencies] ${mode}: raw exit=${raw.status}, signal=${raw.signal}, error=${raw.error}; logs=${logRoot}\n`,
	);
	for (const file of [stdoutPath, stderrPath])
		requireCondition(
			statSync(file).size <= 16 * 1024 * 1024,
			'Knip report exceeds parsing bound; raw output was preserved',
		);
	const decoder = new TextDecoder('utf-8', { fatal: true });
	return {
		...raw,
		stdout: decoder.decode(readFileSync(stdoutPath)),
		stderr: decoder.decode(readFileSync(stderrPath)),
	};
}

function main() {
	requireCondition(process.argv.length === 2, 'This gate accepts no analysis overrides');
	const root = resolve(dirname(modulePath), '..');
	const logRoot = mkdtempSync(join(tmpdir(), 'orchbee-dependencies-'));
	writeFileSync(
		join(logRoot, 'OWNER.json'),
		`${JSON.stringify({ owner: 'invoking verification task', pid: process.pid, purpose: 'raw dependency analysis and aggregate decision', release: 'invoker removes after final diagnosis/review/recovery consumer' }, null, 2)}\n`,
	);
	process.stdout.write(`[dependencies] raw stdout/stderr/results: ${logRoot}\n`);
	try {
		const before = readRetentionPolicy(root);
		const full = captureKnip(root, logRoot, 'full');
		// Preserve the original full-before-strict prerequisite; a failed full never reaches acceptance.
		requireCondition(
			full.status === 0 && parseRun(full, false).length === 0,
			'Full Knip failed; strict unexecuted',
		);
		const strict = captureKnip(root, logRoot, 'strict');
		const after = readRetentionPolicy(root);
		requireCondition(
			before.fingerprint === after.fingerprint,
			'Retention evidence changed during analysis',
		);
		const result = assessDependencyReports(full, strict, after.expected);
		writeFileSync(
			join(logRoot, 'aggregate.json'),
			`${JSON.stringify({ ...result, inputFingerprint: after.fingerprint }, null, 2)}\n`,
		);
		process.stdout.write(
			`[dependencies] aggregate ACCEPTED; full raw exit=${full.status}; strict raw exit=${strict.status}${strict.status === 1 ? ' (still nonzero; exact reviewed retentions)' : ''}\n`,
		);
	} catch (error) {
		writeFileSync(
			join(logRoot, 'aggregate.json'),
			`${JSON.stringify({ accepted: false, reason: String(error) }, null, 2)}\n`,
		);
		throw error;
	}
}

if (process.argv[1] && resolve(process.argv[1]) === modulePath) {
	try {
		main();
	} catch (error) {
		process.stderr.write(`${String(error)}\n`);
		process.exitCode = 1;
	}
}
