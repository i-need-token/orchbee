import { spawn } from 'node:child_process';
import { closeSync, fstatSync, mkdtempSync, openSync, readSync } from 'node:fs';
import { constants, tmpdir } from 'node:os';
import { join } from 'node:path';
import { StringDecoder } from 'node:string_decoder';
import { stripVTControlCharacters } from 'node:util';

const captureVariable = 'ORCHBEE_CHECK_CAPTURED';
const excerptBytes = 4096;
const noticeLimit = 8;

export function checkVerbose(args) {
	const options = args.filter((arg) => arg !== '--');
	if (options.some((arg) => arg !== '--verbose')) {
		throw new Error(`Unknown check options: ${options.join(' ')}; expected --verbose`);
	}
	return options.includes('--verbose');
}

function exitCodeOf(result) {
	if (result.error !== undefined) {
		return 1;
	}
	if (result.signal !== null && result.signal !== undefined) {
		return 128 + (constants.signals[result.signal] ?? 1);
	}
	if (result.status === 0 && result.forwardingError !== undefined) {
		return 1;
	}
	return result.status ?? 1;
}

// Parse only advisory summaries. Bounded lines/chunks keep memory independent of log size.
function summarizeLog(fd) {
	const totals = { passed: 0, failed: 0, skipped: 0, cancelled: 0, todo: 0 };
	let recognized = false;
	let noticeCount = 0;
	const notices = [];
	const diagnostics = [];
	function inspect(rawLine) {
		const line = stripVTControlCharacters(rawLine);
		if (
			diagnostics.length < noticeLimit &&
			/\b(?:[Ee]rror(?:\[[^\]]+\])?:|ERR_[A-Z_]+|FAIL(?:ED)?\b|not ok \d+)/u.test(line)
		) {
			diagnostics.push(line.slice(0, 240));
		}
		const rust = line.match(/test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;/u);
		const node = line.match(/# (pass|fail|skipped|cancelled|todo) (\d+)\s*$/u);
		const zig = line.match(/Build Summary:.*;\s*(\d+)(?:\/\d+)? tests passed(?: \(([^)]*)\))?/u);
		if (rust !== null) {
			recognized = true;
			totals.passed += Number(rust[1]);
			totals.failed += Number(rust[2]);
			totals.skipped += Number(rust[3]);
		} else if (node !== null) {
			recognized = true;
			const key = {
				pass: 'passed',
				fail: 'failed',
				skipped: 'skipped',
				cancelled: 'cancelled',
				todo: 'todo',
			}[node[1]];
			totals[key] += Number(node[2]);
		} else if (zig !== null) {
			recognized = true;
			totals.passed += Number(zig[1]);
			for (const match of (zig[2] ?? '').matchAll(/(\d+) (failed|skipped)\b/gu)) {
				totals[match[2]] += Number(match[1]);
			}
		} else if (/\bTests\s+\d/u.test(line)) {
			for (const match of line.matchAll(/(\d+) (passed|failed|skipped|todo)\b/gu)) {
				recognized = true;
				totals[match[2]] += Number(match[1]);
			}
		}
		if (
			/(?:\b(?:warn(?:ing)?(?:\[[^\]]+\])?\)?:|\w+Warning:|WARN\s|Skipping\b|not applicable on this platform)|# SKIP\b)/iu.test(
				line,
			)
		) {
			noticeCount += 1;
			if (notices.length < noticeLimit) {
				notices.push(line.length > 240 ? `${line.slice(0, 240)}…` : line);
			}
		}
	}
	const chunk = Buffer.alloc(16_384);
	const decoder = new StringDecoder('utf8');
	let pending = '';
	let size;
	while ((size = readSync(fd, chunk, 0, chunk.length, null)) > 0) {
		const parts = decoder.write(chunk.subarray(0, size)).split('\n');
		for (let index = 0; index < parts.length; index += 1) {
			pending = (pending + parts[index]).slice(0, 8192);
			if (index < parts.length - 1) {
				inspect(pending);
				pending = '';
			}
		}
	}
	inspect(pending + decoder.end());
	const summary = recognized
		? `; reported tests: ${totals.passed} passed, ${totals.failed} failed, ${totals.skipped} skipped${totals.cancelled > 0 ? `, ${totals.cancelled} cancelled` : ''}${totals.todo > 0 ? `, ${totals.todo} todo` : ''}`
		: '';
	return { summary, notices, diagnostics, omittedNotices: noticeCount - notices.length };
}

function failureExcerpt(fd) {
	const size = fstatSync(fd).size;
	function readAt(position, length) {
		const buffer = Buffer.alloc(length);
		const bytes = readSync(fd, buffer, 0, length, position);
		return stripVTControlCharacters(buffer.subarray(0, bytes).toString('utf8'));
	}
	if (size <= excerptBytes * 2) {
		const content = readAt(0, size);
		const lines = content.split('\n');
		if (lines.length <= 48) {
			return content;
		}
		return `${lines.slice(0, 24).join('\n')}\n... output omitted; see full log ...\n${lines.slice(-24).join('\n')}`;
	}
	const head = readAt(0, excerptBytes).split('\n').slice(0, 24).join('\n');
	const tail = readAt(size - excerptBytes, excerptBytes)
		.split('\n')
		.slice(-24)
		.join('\n');
	return `${head}\n... output omitted; see full log ...\n${tail}`;
}

function isConcurrent(entry) {
	return entry !== null && typeof entry === 'object' && Array.isArray(entry.concurrent);
}

function childEnv() {
	return { ...process.env, [captureVariable]: '1' };
}

/** Resolves only after the process and its stdio streams have fully drained. */
function awaitClose(child) {
	return new Promise((resolve) => {
		let launchError;
		child.once('error', (error) => {
			launchError = error;
		});
		child.once('close', (status, signal) => {
			resolve({ status, signal, error: launchError, finished: performance.now() });
		});
	});
}

/** Bounded labeled fragments preserve text while destination writes apply backpressure. */
async function forwardPipes(child, label) {
	const pump = async (stream, destination) => {
		stream.setEncoding('utf8');
		let pending = '';
		const write = (fragment) =>
			new Promise((resolve, reject) => {
				destination.write(`[${label}] ${fragment}\n`, (error) => {
					if (error) reject(error);
					else resolve();
				});
			});
		for await (const chunk of stream) {
			pending += chunk;
			while (pending !== '') {
				const newline = pending.indexOf('\n');
				if (newline >= 0 && newline <= 8192) {
					await write(pending.slice(0, newline));
					pending = pending.slice(newline + 1);
				} else if (pending.length > 8192) {
					// Keep a UTF-16 surrogate pair together at the fragment boundary.
					const last = pending.charCodeAt(8191);
					const end = last >= 0xd800 && last <= 0xdbff ? 8191 : 8192;
					await write(pending.slice(0, end));
					pending = pending.slice(end);
				} else {
					break;
				}
			}
		}
		if (pending !== '') await write(pending);
	};
	const results = await Promise.allSettled([
		pump(child.stdout, process.stdout),
		pump(child.stderr, process.stderr),
	]);
	return results.find((result) => result.status === 'rejected')?.reason;
}

async function reportOutcome(outcome) {
	const { step, logPath, started, result } = outcome;
	const exitCode = exitCodeOf(result);
	for (const error of [result.error, result.forwardingError]) {
		if (error !== undefined) {
			process.stderr.write(`${step.name}: ${error.message}\n`);
		}
	}
	if (logPath !== undefined) {
		const logFd = openSync(logPath, 'r');
		try {
			const { summary, notices, diagnostics, omittedNotices } = summarizeLog(logFd);
			const duration = ((result.finished - started) / 1000).toFixed(1);
			const reason = exitCode === 0 ? '' : `; ${result.signal ?? `exit ${exitCode}`}`;
			process.stdout.write(
				`${exitCode === 0 ? 'PASS' : 'FAIL'} ${step.name} (${duration}s${reason}${summary})\n`,
			);
			for (const notice of notices) {
				process.stdout.write(`  ${notice}\n`);
			}
			if (omittedNotices > 0) {
				process.stdout.write(`  ... ${omittedNotices} more notices; see full log\n`);
			}
			if (exitCode !== 0) {
				const excerpt = failureExcerpt(logFd);
				const missingDiagnostics = diagnostics.filter((line) => !excerpt.includes(line));
				process.stderr.write(`Log: ${logPath}\n`);
				if (missingDiagnostics.length > 0) {
					process.stderr.write(
						`Diagnostics from omitted output:\n${missingDiagnostics.join('\n')}\n`,
					);
				}
				process.stderr.write(`${excerpt}\n`);
			}
		} finally {
			closeSync(logFd);
		}
	}
	return exitCode;
}

/**
 * Runs a fixed gate; steps sharing a `{ concurrent: [...] }` entry run in
 * parallel while entries themselves stay ordered and fail-fast.
 * Presentation never determines a check's result.
 */
export async function runChecks({ name, steps, cwd, verbose = false, logRoot = tmpdir() }) {
	try {
		const capture = !verbose && process.env[captureVariable] !== '1';
		const directory = capture ? mkdtempSync(join(logRoot, 'orchbee-checks-')) : undefined;
		if (capture) {
			process.stdout.write(`[${name}] Logs: ${directory}\n`);
		}
		const total = steps.reduce(
			(count, entry) => count + (isConcurrent(entry) ? entry.concurrent.length : 1),
			0,
		);
		const context = { cwd, capture, directory, verbose, total, ordinal: 0 };
		for (const entry of steps) {
			const group = isConcurrent(entry) ? entry.concurrent : [entry];
			let firstFailure = 0;
			const outcomes = [];
			for (const step of group) {
				context.ordinal += 1;
				const ordinal = context.ordinal;
				const logPath = capture
					? join(
							directory,
							`${String(ordinal).padStart(2, '0')}-${step.name.replaceAll(/[^\w.-]/gu, '-')}.log`,
						)
					: undefined;
				const started = performance.now();
				if (capture) {
					process.stdout.write(
						`RUN [${ordinal}/${total}] ${step.name}${group.length > 1 ? ' (concurrent)' : ''}\n`,
					);
				}
				const piped = !capture && group.length > 1;
				const fd = capture ? openSync(logPath, 'wx', 0o600) : undefined;
				let forwarding;
				let child;
				try {
					let stdio = 'inherit';
					if (capture) stdio = ['ignore', fd, fd];
					else if (piped) stdio = ['ignore', 'pipe', 'pipe'];
					child = spawn(step.command, step.args, {
						cwd,
						env: childEnv(),
						stdio,
					});
				} catch (error) {
					if (fd !== undefined) {
						closeSync(fd);
					}
					outcomes.push({ step, logPath, started, result: { error, finished: performance.now() } });
					continue;
				}
				if (piped) {
					forwarding = forwardPipes(child, step.name);
				}
				outcomes.push({ step, logPath, started, fd, child, forwarding, pending: true });
			}
			await Promise.all(
				outcomes.map(async (outcome) => {
					if (!outcome.pending) {
						return;
					}
					const result = await awaitClose(outcome.child);
					if (outcome.fd !== undefined) {
						closeSync(outcome.fd);
					}
					const forwardingError = await outcome.forwarding;
					outcome.result = { ...result, forwardingError };
				}),
			);
			for (const outcome of outcomes) {
				const exitCode = await reportOutcome(outcome);
				if (exitCode !== 0 && firstFailure === 0) {
					firstFailure = exitCode;
				}
			}
			if (firstFailure !== 0) {
				return firstFailure;
			}
		}
		return 0;
	} catch (error) {
		process.stderr.write(`${name}: ${error.message}\n`);
		return 1;
	}
}
