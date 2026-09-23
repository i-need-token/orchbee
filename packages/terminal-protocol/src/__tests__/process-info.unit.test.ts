import { describe, expect, it } from 'vitest';

import { encodeControlRequest } from '../encodeControlRequest';
import { MessageKind } from '../message-contract';
import { TerminalProcessInfoSchema } from '../process-info';

const row = {
	pid: 41,
	parentPid: 17,
	creationTime: '133999999999999999',
	image: 'C:\\Windows\\System32\\cmd.exe',
	commandLine: 'cmd /C echo 中文 😀',
	currentDirectory: null,
};
const sample = {
	attachmentId: '01'.repeat(16),
	terminalId: '02'.repeat(16),
	kind: 'windows-job',
	rootPid: 41,
	processes: [row],
};

describe('read-capable terminal process samples', () => {
	it('admits the exact request and preserves nullable facts and FILETIME', () => {
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
	it('rejects missing nullable fields, duplicate PIDs, invalid text, and invalid identity', () => {
		for (const field of ['parentPid', 'image', 'commandLine', 'currentDirectory']) {
			const partial = Object.fromEntries(Object.entries(row).filter(([key]) => key !== field));
			expect(TerminalProcessInfoSchema.safeParse({ ...sample, processes: [partial] }).success).toBe(
				false,
			);
		}
		for (const bad of [
			{ ...sample, processes: [row, row] },
			{ ...sample, attachmentId: '0'.repeat(32) },
			{ ...sample, kind: 'foreground' },
			{ ...sample, rootPid: 0 },
			...['0', '01', '18446744073709551616'].map((creationTime) => ({
				...sample,
				processes: [{ ...row, creationTime }],
			})),
			...['\ud800', 'x\0y', '😀'.repeat(2048) + 'x'].map((commandLine) => ({
				...sample,
				processes: [{ ...row, commandLine }],
			})),
		])
			expect(TerminalProcessInfoSchema.safeParse(bad).success).toBe(false);
		expect(
			TerminalProcessInfoSchema.safeParse({
				...sample,
				processes: [{ ...row, commandLine: '😀'.repeat(2048) }],
			}).success,
		).toBe(true);
		for (const count of [0, 1, 64, 65]) {
			const processes = Array.from({ length: count }, (_, index) => ({ ...row, pid: index + 1 }));
			expect(TerminalProcessInfoSchema.safeParse({ ...sample, processes }).success).toBe(
				count <= 64,
			);
		}
	});
});
