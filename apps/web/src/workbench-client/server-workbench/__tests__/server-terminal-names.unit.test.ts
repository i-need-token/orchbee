import { createOrchbeeIntl, type IntlShape } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { nameServerTerminalProgram, nameServerTerminalTitle } from '../server-terminal-names';

// The same cases as the TUI's terminal title tests, so both Clients give a terminal one name. Each
// result stays beside its input, so a failure names the case.
const EN_US = createOrchbeeIntl('en-US');
const ZH_CN = createOrchbeeIntl('zh-CN');

function named(
	name: (value: string, intl: IntlShape) => string | null,
	values: readonly string[],
	intl: IntlShape,
): readonly (readonly [string, string | null])[] {
	return values.map((value) => [value, name(value, intl)]);
}

describe('nameServerTerminalTitle', () => {
	it('drops one elevation prefix and one bounded activity mark', () => {
		const toTask = [
			'⠋ task',
			'✳ task',
			'  ⠙   task  ',
			'✢ task',
			'✻ task',
			'◐ task',
			'◓ task',
			'◑ task',
			'◒ task',
			'Administrator:   ⠋ task',
			'管理员: ⠋ task',
			'管理员：task',
		];
		expect(named(nameServerTerminalTitle, toTask, EN_US)).toEqual(
			toTask.map((raw) => [raw, 'task']),
		);
		const kept = [
			['⠋ ⠙ task', '⠙ task'],
			['Administrator: Administrator: task', 'Administrator: task'],
			['管理员: Administrator: task', 'Administrator: task'],
			['管理员:task', '管理员:task'],
			['管理员 task', '管理员 task'],
			['★task', '★task'],
			['✨ task', '✨ task'],
			['@ task', '@ task'],
			['task ⠋ detail', 'task ⠋ detail'],
			['⠋task', '⠋task'],
			[' ⠋ 修复🙂标题 ', '修复🙂标题'],
		] as const;
		expect(
			named(
				nameServerTerminalTitle,
				kept.map(([raw]) => raw),
				EN_US,
			),
		).toEqual(kept);
		const empty = ['', '  ', '⠋   ', 'Administrator: ', '管理员： '];
		expect(named(nameServerTerminalTitle, empty, EN_US)).toEqual(empty.map((raw) => [raw, null]));
	});

	it('names the program of a console default title and keeps a program title as written', () => {
		const programs = [
			['C:\\Program Files\\PowerShell\\7\\pwsh.exe', 'PowerShell', 'PowerShell'],
			[
				'C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe',
				'Windows PowerShell',
				'Windows PowerShell',
			],
			['Administrator: C:\\WINDOWS\\system32\\cmd.exe', 'Command Prompt', '命令提示符'],
			['管理员: C:\\WINDOWS\\system32\\cmd.exe', 'Command Prompt', '命令提示符'],
			[
				'C:\\WINDOWS\\system32\\cmd.exe - ping  localhost',
				'Command Prompt - ping  localhost',
				'命令提示符 - ping  localhost',
			],
			['C:\\My - Tools\\app.exe - run', 'app - run', 'app - run'],
			['C:\\Program Files\\Git\\bin\\BASH.EXE', 'Bash', 'Bash'],
			['c:/tools/build.CMD', 'build', 'build'],
			['\\\\server\\share\\tool.exe', 'tool', 'tool'],
			['⠋ C:\\Windows\\System32\\wsl.exe', 'wsl', 'wsl'],
		] as const;
		const titles = programs.map(([raw]) => raw);
		expect(named(nameServerTerminalTitle, titles, EN_US)).toEqual(
			programs.map(([raw, enUS]) => [raw, enUS]),
		);
		expect(named(nameServerTerminalTitle, titles, ZH_CN)).toEqual(
			programs.map(([raw, , zhCN]) => [raw, zhCN]),
		);
		const written = [
			'C:\\Users\\luna\\project',
			'C:\\Users\\luna\\notes.txt',
			'vim pwsh.exe',
			'PS C:\\work\\app.exe',
			'pwsh',
			'Command Prompt - ping',
			'make test',
		];
		expect(named(nameServerTerminalTitle, written, ZH_CN)).toEqual(
			written.map((raw) => [raw, raw]),
		);
	});
});

describe('nameServerTerminalProgram', () => {
	it('names a launched program by one rule on every platform', () => {
		const programs = [
			['/bin/zsh', 'Zsh', 'Zsh'],
			['/usr/bin/bash', 'Bash', 'Bash'],
			['/opt/homebrew/bin/fish', 'fish', 'fish'],
			['/usr/local/bin/node', 'node', 'node'],
			['pwsh', 'PowerShell', 'PowerShell'],
			['C:\\Program Files\\PowerShell\\7\\PWSH.EXE', 'PowerShell', 'PowerShell'],
			['cmd.exe', 'Command Prompt', '命令提示符'],
			['C:\\tools\\node.exe', 'node', 'node'],
		] as const;
		const paths = programs.map(([path]) => path);
		expect(named(nameServerTerminalProgram, paths, EN_US)).toEqual(
			programs.map(([path, enUS]) => [path, enUS]),
		);
		expect(named(nameServerTerminalProgram, paths, ZH_CN)).toEqual(
			programs.map(([path, , zhCN]) => [path, zhCN]),
		);
		const unnamed = ['', '  ', 'C:\\tools\\', '/usr/bin/'];
		expect(named(nameServerTerminalProgram, unnamed, EN_US)).toEqual(
			unnamed.map((path) => [path, null]),
		);
	});
});
