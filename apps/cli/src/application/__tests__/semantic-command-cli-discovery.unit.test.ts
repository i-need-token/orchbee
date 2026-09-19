import { describe, expect, it, vi } from 'vitest';

import { parseSemanticCommandCliArguments } from '../parseSemanticCommandCliArguments';
import {
	SEMANTIC_COMMAND_CLI_COMMANDS,
	SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	type SemanticCommandCliGrammarCommand,
	type SemanticCommandCliGrammarOption,
} from '../semantic-command-cli-grammar';
import {
	runSemanticCommandCli,
	type SemanticCommandCliDependencies,
} from '../runSemanticCommandCli';

const ALL_OPTION_NAMES = [
	...new Set(
		[
			...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
			...SEMANTIC_COMMAND_CLI_COMMANDS.flatMap((command) => command.options),
		].map((option) => option.name),
	),
];

const OPERAND_VALUES: Record<string, string> = {
	'<operation-id>': 'workbench.project.list',
	'<presentation-id>': 'presentation-1',
	'<request-or-execution-id>': 'cli:request:1',
	'<session-id>': 'session-1',
	'<shell>': 'bash',
};

const OPTION_VALUES: Record<string, string> = {
	'after-revision': '3',
	cursor: 'cursor-1',
	directory: 'directory-1',
	'input-file': 'input.json',
	limit: '5',
	output: 'text',
	project: 'project-1',
	'request-id': 'cli:request:drift',
	rows: '10',
	tab: 'tab-1',
	target: 'ab'.repeat(16),
	timeout: '1000',
	until: 'idle',
	workspace: 'workspace-1',
};

function canonicalArguments(
	command: SemanticCommandCliGrammarCommand,
	location: 'standalone' | 'workspace' = 'workspace',
): string[] {
	const arguments_: string[] = [...command.path];
	if (command.operand !== undefined) {
		arguments_.push(OPERAND_VALUES[command.operand] ?? 'operand-1');
	}
	for (const option of command.options) {
		if (command.location && option.location === true) {
			if (location === 'standalone' && option.name !== 'standalone') continue;
			if (location === 'workspace' && option.name === 'standalone') continue;
		}
		if (option.takesValue) {
			arguments_.push(`--${option.name}`, OPTION_VALUES[option.name] ?? 'value-1');
		} else {
			arguments_.push(`--${option.name}`);
		}
	}
	return arguments_;
}

function dependencies(): SemanticCommandCliDependencies & {
	readonly connect: ReturnType<typeof vi.fn>;
	readonly createRequestIdentity: ReturnType<typeof vi.fn>;
	readonly readInput: ReturnType<typeof vi.fn>;
} {
	return {
		connect: vi.fn<SemanticCommandCliDependencies['connect']>(),
		createRequestIdentity: vi.fn<SemanticCommandCliDependencies['createRequestIdentity']>(),
		readInput: vi.fn<SemanticCommandCliDependencies['readInput']>(),
	};
}

const OFFLINE_ARGV: readonly (readonly string[])[] = [
	[],
	['-h'],
	['--help'],
	['help'],
	['help', 'agent'],
	['agent', 'get', '--help'],
	['--version'],
	['completion', 'bash'],
	['completion', 'zsh'],
	['completion', 'fish'],
	['--skill'],
];

describe('offline discovery grammar', () => {
	it('parses the root help forms to the identical topic', () => {
		for (const arguments_ of [[], ['-h'], ['--help'], ['help']]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				command: { kind: 'help', topic: { kind: 'root' } },
				kind: 'parsed',
			});
		}
	});

	it('parses help topics for families and complete paths', () => {
		const cases = [
			{ arguments: ['help', 'status'], topicKind: 'command', kind: 'status' },
			{ arguments: ['help', 'agent'], topicKind: 'family', name: 'agent' },
			{ arguments: ['help', 'terminal'], topicKind: 'family', name: 'terminal' },
			{ arguments: ['help', 'agent', 'wait'], topicKind: 'command', kind: 'agent-wait' },
			{
				arguments: ['help', 'operation', 'invoke'],
				topicKind: 'command',
				kind: 'operation-invoke',
			},
			{ arguments: ['help', 'completion'], topicKind: 'command', kind: 'completion' },
			{ arguments: ['agent', '--help'], topicKind: 'family', name: 'agent' },
			{ arguments: ['status', '-h'], topicKind: 'command', kind: 'status' },
			{ arguments: ['agent', 'get', '--help'], topicKind: 'command', kind: 'agent-get' },
			{ arguments: ['terminal', 'read', '--help'], topicKind: 'command', kind: 'terminal-read' },
			{ arguments: ['completion', '--help'], topicKind: 'command', kind: 'completion' },
		] as const;
		for (const candidate of cases) {
			const parsed = parseSemanticCommandCliArguments(candidate.arguments);
			expect(parsed.kind).toBe('parsed');
			if (parsed.kind !== 'parsed') continue;
			expect(parsed.command.kind).toBe('help');
			if (parsed.command.kind !== 'help') continue;
			expect(parsed.command.topic).toMatchObject(
				candidate.topicKind === 'command'
					? { command: { kind: candidate.kind }, kind: 'command' }
					: { kind: 'family', name: candidate.name },
			);
		}
	});

	it('parses sole --version and completion shells', () => {
		expect(parseSemanticCommandCliArguments(['--version'])).toEqual({
			command: { kind: 'version' },
			kind: 'parsed',
		});
		for (const shell of ['bash', 'zsh', 'fish'] as const) {
			expect(parseSemanticCommandCliArguments(['completion', shell])).toEqual({
				command: { kind: 'completion', shell },
				kind: 'parsed',
			});
		}
	});

	it('rejects unknown help paths, misplaced flags, and options on offline commands', () => {
		for (const arguments_ of [
			['help', 'frobnicate'],
			['help', 'help'],
			['help', 'version'],
			['help', '--skill'],
			['help', '--version'],
			['help', 'agent', 'get', 'extra'],
			['help', 'agent', 'frobnicate'],
			['agent', 'get', 'session-1', '--help'],
			['status', 'extra', '--help'],
			['agent', '--help', 'get'],
			['operation', 'describe', '--version'],
			['-h', 'status'],
			['--help', 'status'],
			['completion', 'bash', '--help'],
			['completion', 'bash', 'extra'],
			['--version', 'extra'],
			['--version', '--output', 'json'],
			['completion'],
			['completion', 'powershell'],
			['help', '--output', 'json'],
			['--target', 'ab'.repeat(16), '--help'],
			['--input-file', '-', '--help'],
			['terminal', 'read', 'presentation-1', '--rows', '10', '--help'],
			['status', '--help', 'extra'],
		]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				code: 'cli-usage-invalid',
				kind: 'error',
			});
		}
	});

	it('emits byte-identical help for the topic and suffix forms', async () => {
		const dependencies_ = dependencies();
		const helpForm = await runSemanticCommandCli(['help', 'agent', 'get'], dependencies_);
		const longSuffix = await runSemanticCommandCli(['agent', 'get', '--help'], dependencies_);
		const shortSuffix = await runSemanticCommandCli(['agent', 'get', '-h'], dependencies_);
		expect(helpForm.stdout).toBe(longSuffix.stdout);
		expect(longSuffix.stdout).toBe(shortSuffix.stdout);

		const roots = await Promise.all(
			[[], ['-h'], ['--help'], ['help']].map((arguments_) =>
				runSemanticCommandCli(arguments_, dependencies()),
			),
		);
		for (const result of roots) expect(result.stdout).toBe(roots[0]?.stdout);
	});

	it('answers every offline form with exit 0, empty stderr, and zero side effects', async () => {
		for (const arguments_ of OFFLINE_ARGV) {
			const dependencies_ = dependencies();
			const result = await runSemanticCommandCli([...arguments_], dependencies_);
			expect(result.exitCode).toBe(0);
			expect(result.stderr).toBe('');
			expect(result.stdout.endsWith('\n')).toBe(true);
			expect(result.stdout.length).toBeGreaterThan(0);
			expect(dependencies_.connect).not.toHaveBeenCalled();
			expect(dependencies_.createRequestIdentity).not.toHaveBeenCalled();
			expect(dependencies_.readInput).not.toHaveBeenCalled();
		}
	});

	it('prints the build package version as one locale-independent line', async () => {
		for (const locale of ['en-US', 'zh-CN'] as const) {
			const result = await runSemanticCommandCli(['--version'], {
				...dependencies(),
				locale,
			});
			expect(result).toEqual({ exitCode: 0, stderr: '', stdout: 'orchbee-cli 0.1.0\n' });
			expect(result.stdout).not.toContain('"cliVersion"');
		}
	});

	it('localizes help by injected locale and keeps completion grammar words', async () => {
		const english = await runSemanticCommandCli(['help', 'agent'], {
			...dependencies(),
			locale: 'en-US',
		});
		const chinese = await runSemanticCommandCli(['help', 'agent'], {
			...dependencies(),
			locale: 'zh-CN',
		});
		expect(english.stdout).toContain('orchbee-cli agent');
		expect(english.stdout).toContain('agent wait');
		expect(chinese.stdout).toContain('orchbee-cli agent');
		expect(chinese.stdout).toContain('命令：');
		expect(chinese.stdout).not.toBe(english.stdout);

		for (const shell of ['bash', 'zsh', 'fish'] as const) {
			for (const locale of ['en-US', 'zh-CN'] as const) {
				const result = await runSemanticCommandCli(['completion', shell], {
					...dependencies(),
					locale,
				});
				expect(result.exitCode).toBe(0);
				expect(result.stdout).toContain('orchbee-cli');
				expect(result.stdout).toContain('agent');
				expect(result.stdout).toContain('terminal');
			}
		}
	});

	it('root help enumerates every command path and the skill footer', async () => {
		const result = await runSemanticCommandCli(['--help'], dependencies());
		for (const command of SEMANTIC_COMMAND_CLI_COMMANDS) {
			expect(result.stdout).toContain(command.path.join(' '));
		}
		expect(result.stdout).toContain('--skill');
		expect(result.stdout).toContain('--version');
		expect(result.stdout).toContain('--output');
		expect(result.stdout).toContain('--target');
	});
});

describe('grammar table ↔ parser agreement', () => {
	it('table registers exactly the shipped command surface', () => {
		expect(SEMANTIC_COMMAND_CLI_COMMANDS.map((command) => command.kind)).toEqual([
			'status',
			'operation-list',
			'operation-describe',
			'operation-invoke',
			'execution-get',
			'execution-wait',
			'execution-cancel',
			'agent-list',
			'agent-get',
			'agent-explain',
			'agent-changes',
			'agent-wait',
			'project-list',
			'directory-list',
			'workspace-list',
			'tab-list',
			'panel-list',
			'terminal-list',
			'terminal-read',
			'terminal-create',
			'completion',
		]);
	});

	it('every table command parses to its declared kind with declared options', () => {
		for (const command of SEMANTIC_COMMAND_CLI_COMMANDS) {
			if (!command.coordinator) continue;
			const parsed = parseSemanticCommandCliArguments(canonicalArguments(command));
			expect(parsed).toMatchObject({
				command: { kind: command.kind },
				kind: 'parsed',
			});
		}
	});

	it('every undeclared option is rejected on every table command', () => {
		for (const command of SEMANTIC_COMMAND_CLI_COMMANDS) {
			if (!command.coordinator) continue;
			const declared = new Set([
				...command.options.map((option) => option.name),
				...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS.map((option) => option.name),
			]);
			for (const name of ALL_OPTION_NAMES) {
				if (declared.has(name)) continue;
				const option: SemanticCommandCliGrammarOption | undefined = [
					...SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
					...SEMANTIC_COMMAND_CLI_COMMANDS.flatMap((candidate) => candidate.options),
				].find((candidate) => candidate.name === name);
				const arguments_ = [...canonicalArguments(command), `--${name}`];
				if (option?.takesValue) arguments_.push(OPTION_VALUES[name] ?? 'value-1');
				expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
					code: 'cli-usage-invalid',
					kind: 'error',
				});
			}
		}
	});

	it('repeatable --until accepts distinct values; single-use options reject repeats', () => {
		const repeated = parseSemanticCommandCliArguments([
			'agent',
			'wait',
			'session-1',
			'--until',
			'idle',
			'--until',
			'failed',
		]);
		expect(repeated).toMatchObject({
			command: { input: { until: ['idle', 'failed'] }, kind: 'agent-wait' },
			kind: 'parsed',
		});
		for (const arguments_ of [
			['agent', 'wait', 'session-1', '--until', 'idle', '--until', 'idle'],
			['agent', 'list', '--limit', '5', '--limit', '6'],
			['status', '--output', 'json', '--output', 'text'],
		]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				code: 'cli-usage-invalid',
				kind: 'error',
			});
		}
	});

	it('required options and the location mutex are enforced before connection', () => {
		for (const arguments_ of [
			['operation', 'invoke', 'workbench.project.list'],
			['directory', 'list'],
			['workspace', 'list'],
			['panel', 'list', '--standalone'],
			['tab', 'list'],
			['tab', 'list', '--standalone', '--project', 'project-1'],
			['tab', 'list', '--project', 'project-1'],
			['tab', 'list', '--standalone', '--workspace', 'workspace-1'],
			['terminal', 'create'],
			['terminal', 'create', '--standalone', '--project', 'project-1'],
		]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				code: 'cli-usage-invalid',
				kind: 'error',
			});
		}
	});

	it('value options consume one following token and reject option-shaped values', () => {
		for (const arguments_ of [
			['directory', 'list', '--project', '--limit'],
			['agent', 'list', '--limit', '--cursor'],
			['status', '--output', '--target'],
		]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				code: 'cli-usage-invalid',
				kind: 'error',
			});
		}
	});
});
