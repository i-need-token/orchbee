import { AgentWaitConditionSchema } from '@orchbee/agent';

export type SemanticCommandCliLocale = 'en-US' | 'zh-CN';

export type SemanticCommandCliShell = 'bash' | 'fish' | 'zsh';

export const SEMANTIC_COMMAND_CLI_SHELLS: readonly SemanticCommandCliShell[] = [
	'bash',
	'zsh',
	'fish',
];

export interface SemanticCommandCliGrammarOption {
	readonly name: string;
	readonly takesValue: boolean;
	readonly required: boolean;
	readonly repeatable: boolean;
	readonly valueName?: string | undefined;
	readonly values?: readonly string[] | undefined;
	readonly location?: boolean | undefined;
}

export interface SemanticCommandCliGrammarCommand {
	readonly kind: string;
	readonly path: readonly string[];
	readonly coordinator: boolean;
	readonly operand?: string | undefined;
	readonly operandValues?: readonly string[] | undefined;
	readonly location: boolean;
	readonly options: readonly SemanticCommandCliGrammarOption[];
}

const flag = (name: string): SemanticCommandCliGrammarOption => ({
	name,
	required: false,
	repeatable: false,
	takesValue: false,
});

const value = (
	name: string,
	valueName: string,
	extra?: Partial<SemanticCommandCliGrammarOption>,
): SemanticCommandCliGrammarOption => ({
	name,
	required: false,
	repeatable: false,
	takesValue: true,
	valueName,
	...extra,
});

const locationValue = (name: string, valueName: string): SemanticCommandCliGrammarOption => ({
	name,
	location: true,
	required: false,
	repeatable: false,
	takesValue: true,
	valueName,
});

const CURSOR = value('cursor', '<cursor>');
const DETACH = flag('detach');
const LIMIT = value('limit', '<n>');
const REQUEST_ID = value('request-id', '<request-id>');
const STANDALONE: SemanticCommandCliGrammarOption = {
	name: 'standalone',
	location: true,
	required: false,
	repeatable: false,
	takesValue: false,
};
const PROJECT_LOCATION = locationValue('project', '<project-id>');
const WORKSPACE_LOCATION = locationValue('workspace', '<workspace-id>');
const TIMEOUT = value('timeout', '<milliseconds>');

export const SEMANTIC_COMMAND_CLI_COMMON_OPTIONS: readonly SemanticCommandCliGrammarOption[] = [
	value('output', 'json|text', { values: ['json', 'text'] }),
	value('target', '<endpoint-id>'),
];

export const SEMANTIC_COMMAND_CLI_COMMANDS: readonly SemanticCommandCliGrammarCommand[] = [
	{
		coordinator: true,
		kind: 'status',
		location: false,
		options: [],
		path: ['status'],
	},
	{
		coordinator: true,
		kind: 'operation-list',
		location: false,
		options: [],
		path: ['operation', 'list'],
	},
	{
		coordinator: true,
		kind: 'operation-describe',
		location: false,
		operand: '<operation-id>',
		options: [],
		path: ['operation', 'describe'],
	},
	{
		coordinator: true,
		kind: 'operation-invoke',
		location: false,
		operand: '<operation-id>',
		options: [value('input-file', '<file|->', { required: true }), DETACH, REQUEST_ID],
		path: ['operation', 'invoke'],
	},
	{
		coordinator: true,
		kind: 'execution-get',
		location: false,
		operand: '<request-or-execution-id>',
		options: [],
		path: ['execution', 'get'],
	},
	{
		coordinator: true,
		kind: 'execution-wait',
		location: false,
		operand: '<request-or-execution-id>',
		options: [],
		path: ['execution', 'wait'],
	},
	{
		coordinator: true,
		kind: 'execution-cancel',
		location: false,
		operand: '<request-or-execution-id>',
		options: [],
		path: ['execution', 'cancel'],
	},
	{
		coordinator: true,
		kind: 'agent-list',
		location: false,
		options: [flag('all'), LIMIT, CURSOR],
		path: ['agent', 'list'],
	},
	{
		coordinator: true,
		kind: 'agent-get',
		location: false,
		operand: '<session-id>',
		options: [],
		path: ['agent', 'get'],
	},
	{
		coordinator: true,
		kind: 'agent-explain',
		location: false,
		operand: '<session-id>',
		options: [],
		path: ['agent', 'explain'],
	},
	{
		coordinator: true,
		kind: 'agent-changes',
		location: false,
		options: [CURSOR, LIMIT, TIMEOUT, DETACH, REQUEST_ID],
		path: ['agent', 'changes'],
	},
	{
		coordinator: true,
		kind: 'agent-wait',
		location: false,
		operand: '<session-id>',
		options: [
			value('until', '<condition>', {
				repeatable: true,
				values: [...AgentWaitConditionSchema.options],
			}),
			value('after-revision', '<revision>'),
			TIMEOUT,
			DETACH,
			REQUEST_ID,
		],
		path: ['agent', 'wait'],
	},
	{
		coordinator: true,
		kind: 'project-list',
		location: false,
		options: [LIMIT, CURSOR],
		path: ['project', 'list'],
	},
	{
		coordinator: true,
		kind: 'directory-list',
		location: false,
		options: [value('project', '<project-id>', { required: true }), LIMIT, CURSOR],
		path: ['directory', 'list'],
	},
	{
		coordinator: true,
		kind: 'workspace-list',
		location: false,
		options: [value('project', '<project-id>', { required: true }), LIMIT, CURSOR],
		path: ['workspace', 'list'],
	},
	{
		coordinator: true,
		kind: 'tab-list',
		location: true,
		options: [STANDALONE, PROJECT_LOCATION, WORKSPACE_LOCATION, LIMIT, CURSOR],
		path: ['tab', 'list'],
	},
	{
		coordinator: true,
		kind: 'panel-list',
		location: true,
		options: [
			STANDALONE,
			PROJECT_LOCATION,
			WORKSPACE_LOCATION,
			value('tab', '<tab-id>', { required: true }),
			LIMIT,
			CURSOR,
		],
		path: ['panel', 'list'],
	},
	{
		coordinator: true,
		kind: 'terminal-list',
		location: true,
		options: [STANDALONE, PROJECT_LOCATION, WORKSPACE_LOCATION, LIMIT, CURSOR],
		path: ['terminal', 'list'],
	},
	{
		coordinator: true,
		kind: 'terminal-read',
		location: false,
		operand: '<presentation-id>',
		options: [value('rows', '<1-1000>'), DETACH, REQUEST_ID],
		path: ['terminal', 'read'],
	},
	{
		coordinator: true,
		kind: 'terminal-create',
		location: true,
		options: [
			STANDALONE,
			PROJECT_LOCATION,
			WORKSPACE_LOCATION,
			value('directory', '<directory-id>'),
			DETACH,
			REQUEST_ID,
		],
		path: ['terminal', 'create'],
	},
	{
		coordinator: false,
		kind: 'completion',
		location: false,
		operand: '<shell>',
		operandValues: SEMANTIC_COMMAND_CLI_SHELLS,
		options: [],
		path: ['completion'],
	},
];

export type SemanticCommandCliHelpTopic =
	| { readonly kind: 'root' }
	| { readonly kind: 'family'; readonly name: string }
	| { readonly kind: 'command'; readonly command: SemanticCommandCliGrammarCommand };

export function resolveSemanticCommandCliHelpTopic(
	path: readonly string[],
): SemanticCommandCliHelpTopic | null {
	if (path.length === 0) return { kind: 'root' };
	if (path.length === 1) {
		const word = path[0];
		if (word === undefined) return null;
		const command = SEMANTIC_COMMAND_CLI_COMMANDS.find(
			(candidate) => candidate.path.length === 1 && candidate.path[0] === word,
		);
		if (command !== undefined) return { command, kind: 'command' };
		if (SEMANTIC_COMMAND_CLI_COMMANDS.some((candidate) => candidate.path[0] === word)) {
			return { kind: 'family', name: word };
		}
		return null;
	}
	if (path.length === 2) {
		const command = SEMANTIC_COMMAND_CLI_COMMANDS.find(
			(candidate) =>
				candidate.path.length === 2 &&
				candidate.path[0] === path[0] &&
				candidate.path[1] === path[1],
		);
		return command === undefined ? null : { command, kind: 'command' };
	}
	return null;
}

export type SemanticCommandCliDiscoveryCommand =
	| { readonly kind: 'help'; readonly topic: SemanticCommandCliHelpTopic }
	| { readonly kind: 'version' }
	| { readonly kind: 'completion'; readonly shell: SemanticCommandCliShell };

export function resolveSemanticCommandCliDiscovery(
	arguments_: readonly string[],
): SemanticCommandCliDiscoveryCommand | null {
	if (arguments_.length === 0) {
		return { kind: 'help', topic: { kind: 'root' } };
	}
	if (
		arguments_.length === 1 &&
		(arguments_[0] === '-h' || arguments_[0] === '--help' || arguments_[0] === 'help')
	) {
		return { kind: 'help', topic: { kind: 'root' } };
	}
	if (arguments_.length === 1 && arguments_[0] === '--version') {
		return { kind: 'version' };
	}
	if (arguments_[0] === 'help') {
		const path = arguments_.slice(1);
		if (path.length === 0) return { kind: 'help', topic: { kind: 'root' } };
		if (path.every((token) => !token.startsWith('-'))) {
			const topic = resolveSemanticCommandCliHelpTopic(path);
			if (topic !== null) return { kind: 'help', topic };
		}
		return null;
	}
	if (arguments_[0] === 'completion' && arguments_.length === 2) {
		const shell = SEMANTIC_COMMAND_CLI_SHELLS.find((candidate) => candidate === arguments_[1]);
		if (shell !== undefined) {
			return { kind: 'completion', shell };
		}
	}
	const last = arguments_[arguments_.length - 1];
	if ((last === '-h' || last === '--help') && arguments_.length >= 2) {
		const path = arguments_.slice(0, -1);
		if (path.every((token) => !token.startsWith('-'))) {
			const topic = resolveSemanticCommandCliHelpTopic(path);
			if (topic !== null) return { kind: 'help', topic };
		}
	}
	return null;
}
