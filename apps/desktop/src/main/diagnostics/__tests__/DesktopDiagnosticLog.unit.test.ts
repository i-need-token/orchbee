// Main writes the same daily files as the native writers, so these hold the privacy grammar, the
// size cap, and the retention rule that the Rust writer's tests hold for it.
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import { DesktopDiagnosticLog } from '../DesktopDiagnosticLog';

const DAY_MILLISECONDS = 86_400_000;
/** The proposal's daily limit, after which one `log-capped` record closes the day. */
const DAILY_LOG_LIMIT_BYTES = 7 * 1024 * 1024;
const folders: string[] = [];

function folder(): string {
	const created = mkdtempSync(path.join(tmpdir(), 'ob-desktop-log-'));
	folders.push(created);
	return created;
}

function dailyFileName(day: number): string {
	return `${new Date(day * DAY_MILLISECONDS).toISOString().slice(0, 10)}.jsonl`;
}

function clockAt(day: number): { localDay: () => number; now: () => number } {
	return { localDay: () => day, now: () => day * DAY_MILLISECONDS + 1234 };
}

afterEach(() => {
	for (const created of folders.splice(0)) rmSync(created, { force: true, recursive: true });
});

describe('DesktopDiagnosticLog', () => {
	it('never writes text outside the token grammar', () => {
		const directory = folder();
		const day = 20_000;
		const record = new DesktopDiagnosticLog(directory, clockAt(day)).recorder();
		// @ts-expect-error A runtime string, such as a value read from the environment, is refused.
		record('info', 'started', { value: String(process.env['PATH']) });
		const excluded = [
			['terminal content', 'PS C:\\> dir\r\n Volume in drive C'],
			['typed input', 'git commit -m "fix login"'],
			['prompt', 'Refactor the payment module please'],
			['environment value', 'OPENAI_API_KEY=sk-proj-value'],
			['file contents', '[profile]\nname = private'],
			['credential', 'https://user:hunter2@example.com'],
			['command', 'C:\\Tools\\agent.exe'],
			['argument', '--api-token=s3cret'],
			['home path', 'C:\\Users\\Person\\notes'],
		] as const;
		for (const [, text] of excluded) {
			record('info', text, { 'Bad Name': 'kept-token', value: text });
		}
		const written = readFileSync(path.join(directory, dailyFileName(day)), 'utf8');
		for (const [label, text] of excluded) {
			expect({ label, leaked: written.includes(text) }).toEqual({ label, leaked: false });
		}
		const lines = written.trimEnd().split('\n').slice(1);
		expect(lines).toHaveLength(excluded.length);
		for (const line of lines) {
			expect(JSON.parse(line)).toMatchObject({
				event: 'invalid',
				fields: { value: 'invalid' },
				level: 'info',
				role: 'desktop-main',
			});
			expect(line).not.toContain('kept-token');
		}
	});

	it('ends a full day with one capped record inside the limit', () => {
		const directory = folder();
		const day = 20_100;
		const file = path.join(directory, dailyFileName(day));
		writeFileSync(file, 'x'.repeat(DAILY_LOG_LIMIT_BYTES - 300 - 1) + '\n');
		const record = new DesktopDiagnosticLog(directory, clockAt(day)).recorder();
		for (let attempt = 0; attempt < 3; attempt += 1) {
			record('info', 'renderer-gone', { count: attempt, reason: 'crashed' });
		}
		const lines = readFileSync(file, 'utf8').trimEnd().split('\n').slice(1);
		expect(lines.map((line) => JSON.parse(line).event)).toEqual(['log-capped']);
		expect(statSync(file).size).toBeLessThanOrEqual(DAILY_LOG_LIMIT_BYTES);
	});

	it('removes only daily files dated more than six days earlier', () => {
		const directory = folder();
		const today = 20_200;
		const kept = [
			dailyFileName(today - 6),
			dailyFileName(today - 1),
			dailyFileName(today + 3),
			'destructive-commands.jsonl',
			'destructive-commands.1.jsonl',
			'2024-02-30.jsonl',
			'notes.jsonl',
		];
		const expired = [dailyFileName(today - 7), dailyFileName(today - 400)];
		for (const name of [...kept, ...expired]) writeFileSync(path.join(directory, name), '');
		new DesktopDiagnosticLog(directory, clockAt(today)).recorder()('info', 'started');
		expect(new Set(readdirSync(directory))).toEqual(new Set([...kept, dailyFileName(today)]));
	});
});
