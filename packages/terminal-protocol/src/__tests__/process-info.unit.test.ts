import { describe, expect, it } from 'vitest';

import { encodeControlRequest } from '../encodeControlRequest';
import { MessageKind } from '../message-contract';
import { TerminalProcessInfoSchema } from '../process-info';

type Row = [number, number | null, string, string, string | null, string | null, string | null];
const row: Row = [
	41,
	17,
	'133999999999999999',
	'attached',
	'C:\\Windows\\System32\\cmd.exe',
	'cmd /C echo 中文 😀',
	null,
];
const opaque: Row = [43, 41, '133999999999999998', 'unknown', null, null, null];
const sample = {
	attachmentId: '01'.repeat(16),
	terminalId: '02'.repeat(16),
	kind: 'windows-job',
	rootPid: 41,
	processes: [row, opaque],
};
const withRow = (index: number, value: unknown) => ({
	...sample,
	processes: [row.map((field, at) => (at === index ? value : field))],
});

describe('read-capable terminal process samples', () => {
	it('admits the exact request and preserves compact rows, opaque rows, and FILETIME', () => {
		expect(
			encodeControlRequest(MessageKind.readProcessInfo, { attachmentId: sample.attachmentId }).ok,
		).toBe(true);
		expect(
			encodeControlRequest(MessageKind.readProcessInfo, {
				attachmentId: sample.attachmentId,
				pid: 41,
			}).ok,
		).toBe(false);
		expect(TerminalProcessInfoSchema.parse(sample)).toEqual(sample);
		expect(TerminalProcessInfoSchema.safeParse({ ...sample, processes: [] }).success).toBe(true);
	});
	it('rejects short or long rows, duplicate PIDs, invalid console, text, and identity', () => {
		for (const bad of [
			{ ...sample, processes: [row.slice(0, 6)] },
			{ ...sample, processes: [[...row, null]] },
			{ ...sample, processes: [row, row] },
			{ ...sample, attachmentId: '0'.repeat(32) },
			{ ...sample, kind: 'foreground' },
			{ ...sample, rootPid: 0 },
			withRow(3, 'foreground'),
			...['0', '01', '18446744073709551616'].map((creationTime) => withRow(2, creationTime)),
			...['\ud800', 'x\0y', '😀'.repeat(2048) + 'x'].map((commandLine) => withRow(5, commandLine)),
		])
			expect(TerminalProcessInfoSchema.safeParse(bad).success).toBe(false);
		expect(TerminalProcessInfoSchema.safeParse(withRow(5, '😀'.repeat(2048))).success).toBe(true);
		for (const count of [0, 1, 256, 257]) {
			const processes = Array.from({ length: count }, (_, index) => [index + 1, ...row.slice(1)]);
			expect(TerminalProcessInfoSchema.safeParse({ ...sample, processes }).success).toBe(
				count <= 256,
			);
		}
	});
});
