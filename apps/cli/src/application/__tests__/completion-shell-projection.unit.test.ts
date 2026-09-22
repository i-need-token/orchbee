import { execFileSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import { renderSemanticCommandCliCompletion } from '../renderSemanticCommandCliCompletion';

interface CompletionProbe {
	readonly expected: readonly string[];
	readonly line: string;
}

const PROBES: readonly CompletionProbe[] = [
	{ expected: ['--output', '--target', '--help', '-h'], line: 'orchbee-cli status ' },
	{
		expected: ['--limit', '--cursor', '--output', '--target'],
		line: 'orchbee-cli agent list --all ',
	},
	{
		expected: ['--limit', '--cursor', '--output', '--target'],
		line: 'orchbee-cli tab list --standalone ',
	},
	{
		expected: ['--workspace', '--limit', '--cursor', '--output', '--target'],
		line: 'orchbee-cli tab list --project p ',
	},
	{ expected: [], line: 'orchbee-cli status --until ' },
	{ expected: [], line: 'orchbee-cli status --frobnicate ' },
	{ expected: [], line: 'orchbee-cli agent get id --help ' },
	{ expected: [], line: 'orchbee-cli agent get a b ' },
	{ expected: [], line: 'orchbee-cli --skill ' },
	{ expected: [], line: 'orchbee-cli status --output -h ' },
	{ expected: [], line: 'orchbee-cli status --output json --output ' },
	{ expected: [], line: 'orchbee-cli agent frobnicate ' },
	{ expected: [], line: 'orchbee-cli help agent get ' },
	{ expected: [], line: 'orchbee-cli agent list --limit ' },
	{
		expected: [
			'needs-attention',
			'failed',
			'succeeded',
			'canceled',
			'interrupted',
			'working',
			'unknown',
			'ended',
			'fresh',
			'stale',
			'disconnected',
		],
		line: 'orchbee-cli agent wait session-1 --until idle --until ',
	},
	{
		expected: ['list', 'get', 'explain', 'changes', 'wait', '--help', '-h'],
		line: 'orchbee-cli agent ',
	},
	{ expected: ['--output', '--target', '--help', '-h'], line: 'orchbee-cli agent get ' },
	{ expected: ['--output', '--target'], line: 'orchbee-cli agent get sess-1 ' },
	{ expected: ['json', 'text'], line: 'orchbee-cli status --output ' },
	{ expected: ['--target'], line: 'orchbee-cli status --output json ' },
	{ expected: ['json', 'text'], line: 'orchbee-cli agent --output ' },
	{
		expected: ['bash', 'zsh', 'fish', '--help', '-h'],
		line: 'orchbee-cli completion ',
	},
	{
		expected: ['list', 'get', 'explain', 'changes', 'wait'],
		line: 'orchbee-cli help agent ',
	},
	{ expected: ['agent'], line: 'orchbee-cli --output json a' },
	{ expected: ['json', 'text'], line: 'orchbee-cli --output ' },
	{ expected: [], line: 'orchbee-cli --bogus ' },
	{ expected: [], line: 'orchbee-cli --output json help ' },
	{ expected: [], line: 'orchbee-cli --output json h' },
	{
		expected: [
			'status',
			'operation',
			'execution',
			'agent',
			'project',
			'directory',
			'workspace',
			'tab',
			'panel',
			'terminal',
			'--target',
			'--input-file',
			'--detach',
			'--request-id',
			'--all',
			'--limit',
			'--cursor',
			'--timeout',
			'--until',
			'--after-revision',
			'--project',
			'--standalone',
			'--workspace',
			'--tab',
			'--rows',
			'--directory',
		],
		line: 'orchbee-cli --output json ',
	},
	{
		expected: [
			'agent',
			'project',
			'directory',
			'workspace',
			'tab',
			'panel',
			'terminal',
			'--output',
			'--target',
			'--request-id',
			'--all',
			'--limit',
			'--timeout',
			'--detach',
			'--project',
			'--standalone',
			'--workspace',
			'--tab',
		],
		line: 'orchbee-cli --cursor x ',
	},
	{
		expected: [
			'agent',
			'--output',
			'--target',
			'--request-id',
			'--timeout',
			'--after-revision',
			'--detach',
		],
		line: 'orchbee-cli --until idle ',
	},
	{
		expected: [
			'tab',
			'panel',
			'terminal',
			'--output',
			'--target',
			'--request-id',
			'--limit',
			'--cursor',
			'--detach',
			'--tab',
			'--directory',
		],
		line: 'orchbee-cli --standalone ',
	},
	{
		expected: [
			'directory',
			'workspace',
			'tab',
			'panel',
			'terminal',
			'--output',
			'--target',
			'--request-id',
			'--limit',
			'--cursor',
			'--detach',
			'--workspace',
			'--tab',
			'--directory',
		],
		line: 'orchbee-cli --project p ',
	},
	{
		expected: [
			'needs-attention',
			'failed',
			'succeeded',
			'canceled',
			'interrupted',
			'working',
			'unknown',
			'ended',
			'fresh',
			'stale',
			'disconnected',
		],
		line: 'orchbee-cli --until idle --until ',
	},
	{ expected: [], line: 'orchbee-cli --cursor --output ' },
	{ expected: [], line: 'orchbee-cli agent list --cursor --output ' },
	{ expected: [], line: 'orchbee-cli agent list --limit --all ' },
	{ expected: [], line: 'orchbee-cli agent wait session-1 --until idle --until idle ' },
	{ expected: [], line: 'orchbee-cli --until idle --until idle ' },
	{ expected: ['interrupted'], line: 'orchbee-cli agent wait session-1 --until idle --until i' },
	{ expected: ['interrupted'], line: 'orchbee-cli --until idle --until i' },
	{ expected: ['list'], line: 'orchbee-cli --output json agent l' },
	{ expected: ['--limit'], line: 'orchbee-cli --output json agent list --l' },
];

function probeAll(
	shell: 'bash' | 'zsh',
	script: string,
	lines: readonly string[],
): readonly (readonly string[])[] {
	// Feed the generated script to the actual shell without sharing a filesystem
	// namespace: Windows bash may run in WSL and cannot source a Win32 path.
	const prelude = shell === 'bash' ? script : `autoload -Uz compinit && compinit -D\n${script}`;
	const probeBody =
		shell === 'bash'
			? `probe() {
  COMP_LINE="$1"; COMP_POINT=\${#COMP_LINE}
  COMP_WORDS=(); read -ra COMP_WORDS <<< "$COMP_LINE"
  [[ "$COMP_LINE" =~ [[:space:]]$ ]] && COMP_WORDS+=("")
  COMP_CWORD=$(( \${#COMP_WORDS[@]} - 1 ))
  _orchbee_cli
  printf '@@ %s\\n' "\${COMPREPLY[*]-}"
  true
}`
			: `probe() {
  words=( \${(z)1} )
  [[ "$1" =~ [[:space:]]$ ]] && words+=("")
  CURRENT=\${#words}
  local out
  out=$(_orchbee_cli)
  local candidate
  for candidate in \${(f)out}; do
    [[ "$candidate" == "\${words[CURRENT]}"* ]] && print -l -- "$candidate"
  done
  true
}`;
	const quoted = lines
		.map(
			(line) => `echo "== ${line.replaceAll('"', '\\"')}"\nprobe "${line.replaceAll('"', '\\"')}"`,
		)
		.join('\n');
	const output = execFileSync(shell, ['-s'], {
		input: `${prelude}\n${probeBody}\n${quoted}\n`,
		encoding: 'utf8',
		timeout: 30_000,
	});
	const results: string[][] = [];
	let current: string[] | undefined;
	for (const row of output.split('\n')) {
		if (row.startsWith('== ')) {
			current = [];
			results.push(current);
		} else if (row.startsWith('@@ ') && current !== undefined) {
			current.push(
				...row
					.slice(3)
					.split(' ')
					.filter((word) => word !== ''),
			);
		} else if (current !== undefined && row !== '') {
			current.push(...row.split(' ').filter((word) => word !== ''));
		}
	}
	return results;
}

function shellAvailable(name: string): boolean {
	try {
		execFileSync(name, ['-c', 'true'], { timeout: 5_000 });
		return true;
	} catch {
		return false;
	}
}

const bashAvailable = shellAvailable('bash');
const zshAvailable = shellAvailable('zsh');

describe('generated shell completion semantics', () => {
	it.skipIf(!bashAvailable)('real bash: every probe emits exactly the legal candidate set', () => {
		const script = renderSemanticCommandCliCompletion('bash');
		const actual = probeAll(
			'bash',
			script,
			PROBES.map((probe) => probe.line),
		);
		expect(actual.map((words) => [...words].sort())).toEqual(
			PROBES.map((probe) => [...probe.expected].sort()),
		);
	});

	it.skipIf(!zshAvailable)('real zsh: every probe emits exactly the legal candidate set', () => {
		const script = renderSemanticCommandCliCompletion('zsh');
		const actual = probeAll(
			'zsh',
			script,
			PROBES.map((probe) => probe.line),
		);
		expect(actual.map((words) => [...words].sort())).toEqual(
			PROBES.map((probe) => [...probe.expected].sort()),
		);
	});

	it('fish script is a static projection with no live lookup', () => {
		const script = renderSemanticCommandCliCompletion('fish');
		expect(script).toContain('complete -f -c orchbee-cli');
		expect(script).toContain('--until');
		expect(script).toContain('bash zsh fish');
		expect(script).toContain("case 'agent list'");
		expect(script).not.toMatch(/"[^"\n]+"\)\s/);
		expect(script).not.toContain(';;');
	});
});
