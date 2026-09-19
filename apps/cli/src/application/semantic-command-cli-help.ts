import {
	SEMANTIC_COMMAND_CLI_COMMANDS,
	SEMANTIC_COMMAND_CLI_COMMON_OPTIONS,
	type SemanticCommandCliGrammarCommand,
	type SemanticCommandCliGrammarOption,
	type SemanticCommandCliHelpTopic,
	type SemanticCommandCliLocale,
} from './semantic-command-cli-grammar';

interface HelpStrings {
	readonly commandDescription: Record<string, string>;
	readonly familyDescription: Record<string, string>;
	readonly lines: {
		readonly commands: string;
		readonly details: string;
		readonly options: string;
		readonly optionsCommon: string;
		readonly skill: string;
		readonly title: string;
		readonly usage: string;
	};
	readonly optionDescription: Record<string, string>;
}

const EN_US: HelpStrings = {
	commandDescription: {
		'agent-changes': 'Follow the global canonical Agent change feed',
		'agent-explain': 'Explain one Session observation basis',
		'agent-get': 'Read one Session snapshot',
		'agent-list': 'List observed Agent Sessions',
		'agent-wait': 'Wait for a canonical Session condition',
		completion: 'Print a static shell completion projection',
		'directory-list': 'List Directories of one Project',
		'execution-cancel': 'Request cooperative cancellation of one execution',
		'execution-get': 'Look up one execution by request or execution identity',
		'execution-wait': 'Wait for one execution to reach a terminal phase',
		'operation-describe': 'Describe one admitted operation',
		'operation-invoke': 'Invoke one admitted operation with a JSON input',
		'operation-list': 'List admitted operations',
		'panel-list': 'List Panels of one Tab at one location',
		'project-list': 'List Projects',
		status: 'Coordinator status',
		'tab-list': 'List Tabs at one location',
		'terminal-create': 'Create one Terminal at one location',
		'terminal-list': 'List Terminals at one location',
		'terminal-read': 'Read bounded tail output of one Terminal',
		'workspace-list': 'List Workspaces of one Project',
	},
	familyDescription: {
		agent: 'Canonical Agent Session observation',
		directory: 'Workbench Directory collection',
		execution: 'Semantic execution observation',
		operation: 'Generic admitted-operation surface',
		panel: 'Workbench Panel layout',
		project: 'Workbench Project collection',
		tab: 'Workbench Tab layout',
		terminal: 'Workbench Terminal layout',
		workspace: 'Workbench Workspace collection',
	},
	lines: {
		commands: 'Commands:',
		details: 'Run "orchbee-cli help <family>" or "orchbee-cli <path> --help" for details.',
		options: 'Options:',
		optionsCommon: 'Common options:',
		skill: 'Agents: run "orchbee-cli --skill" for the embedded Skill.',
		title: 'orchbee-cli — Orchbee semantic command-line client',
		usage: 'Usage:',
	},
	optionDescription: {
		'after-revision': 'Require a strictly newer global Agent runtime revision',
		all: 'Include retained ended Sessions',
		cursor: 'Continue the page from a returned cursor',
		detach: 'Return after acceptance; observe later with execution get|wait',
		directory: 'Directory identity',
		'input-file': 'Read the JSON input from file, or - for stdin',
		limit: 'Page size bound',
		output: 'Document serialization (default json)',
		project: 'Project identity',
		'request-id': 'Caller request identity',
		rows: 'Retained tail rows, 1-1000 (default 80)',
		standalone: 'Standalone (project-free) location',
		tab: 'Tab identity',
		target: 'Exact trusted coordinator endpoint identity',
		timeout: 'Observation deadline in milliseconds',
		until: 'Wait condition; repeatable',
		workspace: 'Workspace identity',
	},
};

const ZH_CN: HelpStrings = {
	commandDescription: {
		'agent-changes': '跟踪 global canonical Agent change feed',
		'agent-explain': '解释一个 Session observation basis',
		'agent-get': '读取一个 Session snapshot',
		'agent-list': '列出已观察的 Agent Session',
		'agent-wait': '等待一个 canonical Session condition',
		completion: '打印静态 shell completion projection',
		'directory-list': '列出一个 Project 的 Directory',
		'execution-cancel': '对一个 execution 请求协作式取消',
		'execution-get': '按 request 或 execution identity 查询一个 execution',
		'execution-wait': '等待一个 execution 到达 terminal phase',
		'operation-describe': '描述一个已准入 operation',
		'operation-invoke': '用一份 JSON input 调用一个已准入 operation',
		'operation-list': '列出已准入 operation',
		'panel-list': '列出一个位置上某个 Tab 的 Panel',
		'project-list': '列出 Project',
		status: 'Coordinator 状态',
		'tab-list': '列出一个位置上的 Tab',
		'terminal-create': '在一个位置上创建一个 Terminal',
		'terminal-list': '列出一个位置上的 Terminal',
		'terminal-read': '读取一个 Terminal 的有界 tail 输出',
		'workspace-list': '列出一个 Project 的 Workspace',
	},
	familyDescription: {
		agent: 'Canonical Agent Session observation',
		directory: 'Workbench Directory collection',
		execution: 'Semantic execution observation',
		operation: '通用已准入 operation 表面',
		panel: 'Workbench Panel layout',
		project: 'Workbench Project collection',
		tab: 'Workbench Tab layout',
		terminal: 'Workbench Terminal layout',
		workspace: 'Workbench Workspace collection',
	},
	lines: {
		commands: '命令：',
		details: '运行 "orchbee-cli help <family>" 或 "orchbee-cli <path> --help" 查看详情。',
		options: '选项：',
		optionsCommon: '公共选项：',
		skill: 'Agent：运行 "orchbee-cli --skill" 获取内嵌 Skill。',
		title: 'orchbee-cli — Orchbee semantic command-line client',
		usage: '用法：',
	},
	optionDescription: {
		'after-revision': '要求一个严格更新的 global Agent runtime revision',
		all: '包含已保留的 ended Session',
		cursor: '从返回的 cursor 继续分页',
		detach: '接受后立即返回；之后用 execution get|wait 观察',
		directory: 'Directory identity',
		'input-file': '从文件读取 JSON input，或 - 表示 stdin',
		limit: '分页大小上限',
		output: 'Document serialization（默认 json）',
		project: 'Project identity',
		'request-id': '调用方 request identity',
		rows: '保留的 tail 行数，1-1000（默认 80）',
		standalone: 'standalone（无 project）位置',
		tab: 'Tab identity',
		target: '精确可信的 coordinator endpoint identity',
		timeout: '观察 deadline（毫秒）',
		until: '等待 condition；可重复',
		workspace: 'Workspace identity',
	},
};

const STRINGS: Record<SemanticCommandCliLocale, HelpStrings> = {
	'en-US': EN_US,
	'zh-CN': ZH_CN,
};

export function renderSemanticCommandCliHelp(
	topic: SemanticCommandCliHelpTopic,
	locale: SemanticCommandCliLocale,
): string {
	const strings = STRINGS[locale];
	if (topic.kind === 'root') return renderRoot(strings);
	if (topic.kind === 'family') return renderFamily(strings, topic.name);
	return renderCommand(strings, topic.command);
}

function optionUsage(option: SemanticCommandCliGrammarOption): string {
	if (!option.takesValue) return ` [--${option.name}]`;
	const valueName = option.valueName ?? `<${option.name}>`;
	return option.required
		? ` --${option.name} ${valueName}`
		: ` [--${option.name} ${valueName}]${option.repeatable ? '...' : ''}`;
}

export function semanticCommandCliUsageLine(command: SemanticCommandCliGrammarCommand): string {
	const base = `orchbee-cli ${command.path.join(' ')}`;
	const operand = command.operand === undefined ? '' : ` ${command.operand}`;
	const location = command.location
		? ' (--standalone | --project <project-id> --workspace <workspace-id>)'
		: '';
	const options = command.options
		.filter((option) => option.location !== true)
		.map(optionUsage)
		.join('');
	return `${base}${operand}${location}${options}`;
}

function renderRoot(strings: HelpStrings): string {
	const lines = [
		strings.lines.title,
		'',
		`${strings.lines.usage} orchbee-cli <command> [options]`,
		'',
		strings.lines.commands,
	];
	for (const command of SEMANTIC_COMMAND_CLI_COMMANDS) {
		lines.push(
			`  ${semanticCommandCliUsageLine(command)}`,
			`      ${strings.commandDescription[command.kind] ?? ''}`,
		);
	}
	lines.push(
		'  orchbee-cli help [<family-or-path>]',
		'  orchbee-cli --skill',
		'  orchbee-cli --version',
		'',
		strings.lines.optionsCommon,
	);
	for (const option of SEMANTIC_COMMAND_CLI_COMMON_OPTIONS) {
		lines.push(`  --${option.name} ${option.valueName ?? ''}`);
	}
	lines.push('', strings.lines.details, strings.lines.skill, '');
	return lines.join('\n');
}

function renderFamily(strings: HelpStrings, name: string): string {
	const commands = SEMANTIC_COMMAND_CLI_COMMANDS.filter((command) => command.path[0] === name);
	const lines = [
		`orchbee-cli ${name} — ${strings.familyDescription[name] ?? ''}`,
		'',
		strings.lines.commands,
	];
	for (const command of commands) {
		lines.push(`  ${semanticCommandCliUsageLine(command)}`);
	}
	lines.push('', strings.lines.details, strings.lines.skill, '');
	return lines.join('\n');
}

function renderCommand(strings: HelpStrings, command: SemanticCommandCliGrammarCommand): string {
	const lines = [
		`${semanticCommandCliUsageLine(command)} — ${strings.commandDescription[command.kind] ?? ''}`,
	];
	if (command.options.length > 0 || command.coordinator) {
		lines.push('', strings.lines.options);
		for (const option of command.options) {
			const values = option.values === undefined ? '' : ` (${option.values.join('|')})`;
			const usage = option.takesValue
				? `--${option.name} ${option.valueName ?? `<${option.name}>`}`
				: `--${option.name}`;
			lines.push(`  ${usage}   ${strings.optionDescription[option.name] ?? ''}${values}`);
		}
	}
	if (command.coordinator) {
		lines.push('', strings.lines.optionsCommon);
		for (const option of SEMANTIC_COMMAND_CLI_COMMON_OPTIONS) {
			lines.push(
				`  --${option.name} ${option.valueName ?? ''}   ${strings.optionDescription[option.name] ?? ''}`,
			);
		}
	}
	lines.push('', strings.lines.skill, '');
	return lines.join('\n');
}
