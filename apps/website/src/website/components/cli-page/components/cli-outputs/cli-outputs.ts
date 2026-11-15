import { CLI_FAMILIES } from '../../cli-families';

// What each command of the output explorer prints, line by line, as the command line writes it in
// its default text form (apps/orchbee/src/cli_text), against the scripted Session's Agents.
// Terminal contents are English in every page language.

/** A run of output: plain text, a JSON key, or a mark that the document is abridged. */
type OutputSegment = string | { readonly tone: 'key' | 'elided'; readonly text: string };
export type OutputLine = readonly OutputSegment[];

export interface CliOutput {
	readonly command: string;
	readonly lines: readonly OutputLine[];
}

const CLAUDE = '@panel:0x2a';
const CODEX = '@panel:0x36';
const OPENCODE = '@panel:0x41';

const AGENTS = [
	{ handle: CLAUDE, agent: 'claude-code', name: '-', state: 'Working', cwd: 'C:\\demo\\shop-api' },
	{ handle: CODEX, agent: 'codex', name: '-', state: 'Idle', cwd: 'C:\\demo\\shop-api' },
	{ handle: OPENCODE, agent: 'opencode', name: '-', state: 'Done', cwd: 'C:\\demo\\shop-web' },
] as const;

/** `orchbee agent list`: six columns, each padded to its width and two spaces apart. */
function agentList(): OutputLine[] {
	const handleWidth = Math.max(...AGENTS.map((agent) => agent.handle.length));
	const nameWidth = Math.min(12, Math.max(4, ...AGENTS.map((agent) => agent.name.length)));
	const widths = [4, handleWidth, 12, nameWidth, 9];
	const line = (cells: readonly string[]): OutputLine => [
		cells.map((cell, index) => cell.padEnd(widths[index] ?? 0)).join('  '),
	];
	return [
		line(['#', 'HANDLE', 'AGENT', 'NAME', 'STATE', 'CWD']),
		...AGENTS.map((agent, index) =>
			line([String(index + 1), agent.handle, agent.agent, agent.name, agent.state, agent.cwd]),
		),
	];
}

const key = (text: string): OutputSegment => ({ tone: 'key', text: `"${text}"` });
const ELIDED: OutputSegment = { tone: 'elided', text: '…' };

// `orchbee agent list --format json` prints its members sorted by name (serde_json without
// preserve_order); an elision mark stands where members the page leaves out would be.

/** One item of the JSON list, with the members the page shows. */
function jsonItem(ordinal: number, provider: string, status: string, cwd: string): OutputLine[] {
	const field = (name: string, value: string): OutputLine => ['        ', key(name), `: ${value},`];
	return [
		['      {'],
		['        ', ELIDED],
		field('cwd', `"${cwd.replaceAll('\\', '\\\\')}"`),
		field('name', 'null'),
		field('ordinal', String(ordinal)),
		['        ', ELIDED],
		field('provider', `"${provider}"`),
		field('source', '"terminal"'),
		field('status', `"${status}"`),
		['        ', ELIDED],
	];
}

function agentListJson(): OutputLine[] {
	return [
		['{'],
		['  ', key('result'), ': {'],
		['    ', ELIDED],
		['    ', key('items'), ': ['],
		...jsonItem(1, 'claude-code', 'working', 'C:\\demo\\shop-api'),
		['      },'],
		['      ', ELIDED],
		['    ],'],
		['    ', key('kind'), ': "listed",'],
		['    ', key('nextCursor'), ': null,'],
		['    ', ELIDED],
		['  },'],
		['  ', key('schema'), ': "orchbee.agent.list",'],
		[
			'  ',
			key('source'),
			': { ',
			ELIDED,
			', ',
			key('kind'),
			': "agent-projection", ',
			ELIDED,
			' },',
		],
		['  ', key('version'), ': 1'],
		['}'],
	];
}

/** Breaks `text` into lines of at most 78 columns, the first after `first`, the rest after `rest`. */
function wrap(text: string, first: string, rest: string): string[] {
	const lines: string[] = [];
	let line = first;
	let empty = true;
	for (const word of text.split(' ')) {
		if (!empty && line.length + 1 + word.length > 78) {
			lines.push(line);
			line = rest + word;
		} else line += (empty ? '' : ' ') + word;
		empty = false;
	}
	lines.push(line);
	return lines;
}

/** `orchbee --help`: each section a 24-column label, two spaces, and text wrapped at 78 columns. */
function help(): OutputLine[] {
	const entry = ([label, text]: readonly [string, string]): string[] =>
		wrap(text, `  ${label.padEnd(24)}  `, ' '.repeat(28));
	const section = (title: string, entries: readonly (readonly [string, string])[]): string[] => [
		'',
		`${title}:`,
		...entries.flatMap(entry),
	];
	const text = [
		'Orchbee',
		'A terminal for running and coordinating AI Agents.',
		'',
		'Usage:',
		'  orchbee [<command>] [<options>]',
		...section('Quick start', [
			['orchbee', 'Open the terminal interface.'],
			['orchbee workspace create', 'Create a workspace and its first terminal.'],
			['orchbee agent list', 'See your active Agents.'],
		]),
		...CLI_FAMILIES.flatMap((group) =>
			section(
				group.title.defaultMessage,
				group.rows.map((row) => [row.command, row.help.defaultMessage] as const),
			),
		),
		...section('Global options', [
			['-h, --help', 'Show this help and exit.'],
			['-v, -V, --version', 'Print the installed version.'],
			['--skill', 'Print the offline Agent guide.'],
			['--session <name>', 'Select a local Orchbee session; default: default.'],
			['--machine <target>', 'Select a saved SSH machine.'],
		]),
		'',
		...wrap('Use orchbee <command> -h or orchbee help <command> for details.', '', ''),
		...wrap(
			'Use --format text|json where supported; see each command for its output form.',
			'',
			'',
		),
		...wrap(
			'Configuration: C:\\Users\\dev\\.orchbee\\config.toml. ORCHBEE_HOME moves the whole data folder; ORCHBEE_CONFIG_FILE moves only the configuration file. Use orchbee config check to inspect it and orchbee doctor to check the whole installation.',
			'',
			'',
		),
	];
	return text.map((line) => [line]);
}

export const CLI_OUTPUTS: readonly CliOutput[] = [
	{ command: 'orchbee agent list', lines: agentList() },
	{
		command: 'orchbee agent start codex --split right',
		lines: [
			[`Started Codex in a new Panel on the right (Panel ${CODEX}).`],
			[`Next: orchbee agent wait --panel ${CODEX} --until idle`],
		],
	},
	{
		command: `orchbee agent prompt ${CODEX} "Review the current changes"`,
		lines: [[`Prompt submitted to ${CODEX}.`]],
	},
	{
		command: `orchbee agent wait ${CODEX} --until idle --timeout 120000`,
		lines: [[`${CODEX} matched idle at state sequence 3 after 8412ms`]],
	},
	{
		command: `orchbee agent read ${CODEX} --source recent --lines 40`,
		lines: [
			['› Review the current changes'],
			['• Ran git diff --stat'],
			['  3 files changed, 41 insertions(+), 9 deletions(-)'],
			['• Read src/orders/routes.ts'],
			['• Read test/orders.test.ts'],
			['  One test still posts quantity 0 on purpose.'],
			['  Suggest updating the fixture in orders.test.ts.'],
		],
	},
	{ command: 'orchbee agent list --format json', lines: agentListJson() },
	{ command: 'orchbee --help', lines: help() },
];
