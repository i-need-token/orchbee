import { z } from 'zod';

/**
 * The startable Agent catalog; the Server resolves an id to the new shell's first command and names
 * it again when that start did not happen.
 */
export const WorkbenchAgentStartProviderSchema = z.enum([
	'pi',
	'claude-code',
	'codex',
	'gemini',
	'cursor',
	'devin',
	'antigravity',
	'cline',
	'omp',
	'mastracode',
	'opencode',
	'github-copilot',
	'kimi',
	'kiro',
	'droid',
	'amp',
	'grok',
	'hermes',
	'kilo',
	'qodercli',
	'qwen-code',
	'letta',
	'maki',
	'muse',
	'atomcode',
	'codebuddy',
	'command-code',
	'deveco',
	'mimo',
	'reasonix',
	'snow',
	'minimax-code',
	'mothx',
	'jcode',
	'openclaw',
	'prime-agent',
]);
export type WorkbenchAgentStartProvider = z.output<typeof WorkbenchAgentStartProviderSchema>;

const MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES = 2048;
const MAXIMUM_CUSTOM_ARGUMENTS = 24;
const utf8 = new TextEncoder();

// The provider flags and argument grammar match the Rust launch settings in workbench-protocol.
const YOLO = {
	antigravity: {
		flag: '--dangerously-skip-permissions',
		long: ['--dangerously-skip-permissions', '--sandbox', '--mode'],
		short: [],
	},
	'claude-code': {
		flag: '--dangerously-skip-permissions',
		long: ['--dangerously-skip-permissions', '--permission-mode'],
		short: [],
	},
	codex: {
		flag: '--dangerously-bypass-approvals-and-sandbox',
		long: [
			'--dangerously-bypass-approvals-and-sandbox',
			'--sandbox',
			'--ask-for-approval',
			'--full-auto',
			'--yolo',
		],
		short: ['-s', '-a'],
	},
	cursor: { flag: '--yolo', long: ['--yolo', '--force', '--sandbox'], short: [] },
	gemini: {
		flag: '--approval-mode=yolo',
		long: ['--approval-mode', '--yolo'],
		short: ['-y'],
	},
	'github-copilot': {
		flag: '--yolo',
		long: [
			'--yolo',
			'--allow-all',
			'--allow-all-tools',
			'--allow-tool',
			'--deny-tool',
			'--assisted-approval',
		],
		short: [],
	},
	grok: {
		flag: '--always-approve',
		long: [
			'--always-approve',
			'--yolo',
			'--permission-mode',
			'--dangerously-skip-permissions',
			'--allow',
			'--deny',
			'--sandbox',
		],
		short: [],
	},
	'qwen-code': {
		flag: '--approval-mode=yolo',
		long: ['--approval-mode', '--yolo', '--sandbox'],
		short: ['-s'],
	},
} as const satisfies Partial<
	Record<
		WorkbenchAgentStartProvider,
		{
			readonly flag: string;
			readonly long: readonly string[];
			readonly short: readonly string[];
		}
	>
>;

export type AgentYoloProvider = keyof typeof YOLO;
export const AGENT_YOLO_PROVIDERS = [
	'antigravity',
	'claude-code',
	'codex',
	'cursor',
	'gemini',
	'github-copilot',
	'grok',
	'qwen-code',
] as const satisfies readonly AgentYoloProvider[];

export function agentYoloFlag(provider: AgentYoloProvider): string {
	return YOLO[provider].flag;
}

export function agentOverridesYolo(provider: AgentYoloProvider, words: readonly string[]): boolean {
	const { long, short } = YOLO[provider];
	for (const word of words) {
		if (word === '--') break;
		const option = word.split('=', 1)[0];
		if (long.some((name) => name === option) || short.some((name) => word.startsWith(name))) {
			return true;
		}
	}
	return false;
}

type AgentArgumentInput =
	| { readonly valid: true; readonly words: readonly string[] }
	| {
			readonly valid: false;
			readonly reason: 'unclosed-quote' | 'invalid-value' | 'value-too-long' | 'too-many-arguments';
	  };

/** Splits literal words. A matching quote may be escaped inside quotes. */
export function parseAgentArguments(input: string): AgentArgumentInput {
	if (utf8.encode(input).byteLength > MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES) {
		return { reason: 'value-too-long', valid: false };
	}
	const words: string[] = [];
	let word = '';
	let started = false;
	let quote: string | null = null;
	let skip = false;
	const characters = [...input];
	for (const [index, character] of characters.entries()) {
		if (skip) {
			skip = false;
			continue;
		}
		if (/\p{Cc}/u.test(character) && (character !== '\t' || quote !== null)) {
			return { reason: 'invalid-value', valid: false };
		}
		if (quote !== null) {
			if (character === quote) quote = null;
			else if (character === '\\' && characters[index + 1] === quote) {
				word += quote;
				skip = true;
			} else word += character;
		} else if (character === ' ' || character === '\t') {
			if (started) words.push(word);
			word = '';
			started = false;
		} else {
			started = true;
			if (character === '"' || character === "'") quote = character;
			else word += character;
		}
	}
	if (quote !== null) return { reason: 'unclosed-quote', valid: false };
	if (started) words.push(word);
	if (words.length > MAXIMUM_CUSTOM_ARGUMENTS)
		return { reason: 'too-many-arguments', valid: false };
	return { valid: true, words };
}
