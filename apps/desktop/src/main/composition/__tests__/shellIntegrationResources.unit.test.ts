import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const resources = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../../resources/shell-integration',
);

describe('Desktop shell integration resources', () => {
	it('ships manual, idempotent Zsh, Bash, and Fish integrations without command text transport', async () => {
		const [zsh, bash, fish] = await Promise.all(
			['orchbee.zsh', 'orchbee.bash', 'orchbee.fish'].map((name) =>
				readFile(path.join(resources, name), 'utf8'),
			),
		);
		for (const source of [zsh, bash, fish]) {
			expect(source).toContain('133;C');
			expect(source).toContain('133;D;');
			expect(source).toContain('133;A');
			expect(source).toContain('file://');
			expect(source).not.toMatch(/cmdline|command[_ -]?text|history/iu);
		}
		expect(bash).toContain('BASH_VERSINFO[1] < 4');
		expect(bash).toContain('unsupported-bash-version');
		expect(zsh).toContain('__ORCHBEE_SHELL_INTEGRATION_LOADED');
		expect(fish).toContain('__orchbee_shell_integration_loaded');
	});

	it.runIf(process.platform !== 'win32')(
		'emits one Zsh command boundary with an encoded working directory and exit code',
		() => {
			const script = [
				`source ${shellQuote(path.join(resources, 'orchbee.zsh'))}`,
				'__orchbee_preexec',
				'false',
				'__orchbee_precmd',
			].join('\n');
			const output = execFileSync('/bin/zsh', ['-f', '-c', script]);
			expect(output.toString('hex')).toMatch(
				/^1b5d3133333b43071b5d3133333b443b31071b5d373b66696c653a2f2f.*071b5d3133333b4107$/u,
			);
		},
	);
});

function shellQuote(value: string): string {
	return `'${value.replaceAll("'", `'\\''`)}'`;
}
