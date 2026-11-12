/**
 * The settings registry's key table, mirrored from `schemas/workbench/v1/setting-keys-v1.json`,
 * which the Rust registry generates. A unit test keeps the two equal; edit the registry and copy the
 * regenerated file here rather than editing this table by hand.
 *
 * @internal Exported for the equality test; consumers read keys through the helpers below.
 */
export const SETTING_KEY_TABLE = {
	contractVersion: 1,
	platforms: ['windows', 'macos', 'linux'],
	keys: [
		{
			path: 'general.language',
			type: 'choice',
			choices: ['system', 'en-US', 'zh-CN'],
			default: 'system',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'general',
			group: 'basics',
			page: true,
		},
		{
			path: 'appearance.mode',
			type: 'choice',
			choices: ['system', 'light', 'dark'],
			default: 'system',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'theme',
			page: true,
		},
		{
			path: 'appearance.light_theme',
			type: 'theme',
			maximumBytes: 64,
			default: 'orchbee-light',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'themes',
			group: 'light',
			page: true,
		},
		{
			path: 'appearance.dark_theme',
			type: 'theme',
			maximumBytes: 64,
			default: 'orchbee-dark',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'themes',
			group: 'dark',
			page: true,
		},
		{
			path: 'appearance.tui_colors',
			type: 'choice',
			choices: ['theme', 'terminal'],
			default: 'theme',
			scope: 'client',
			clients: 'tui',
			effect: 'immediate',
			category: 'appearance',
			group: 'theme',
			page: false,
		},
		{
			path: 'appearance.terminal_colors',
			type: 'choice',
			choices: ['theme', 'host', 'custom'],
			default: 'theme',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'tcolors',
			page: true,
		},
		{
			path: 'appearance.custom_foreground',
			type: 'color',
			default: '#141414',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'tcolors',
			page: true,
		},
		{
			path: 'appearance.custom_background',
			type: 'color',
			default: '#fcfcfc',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'tcolors',
			page: true,
		},
		{
			path: 'appearance.window_title',
			type: 'text',
			maximumBytes: 4096,
			default: '{hostname}: {workspace}',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'general',
			group: 'basics',
			page: false,
		},
		{
			path: 'appearance.sidebar.agents.rows',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'sidebar',
			page: false,
		},
		{
			path: 'appearance.sidebar.agents.rows_by_agent',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'sidebar',
			page: false,
		},
		{
			path: 'appearance.sidebar.agents.row_gap',
			type: 'integer',
			minimum: 0,
			maximum: 65535,
			step: 1,
			default: 0,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'sidebar',
			page: false,
		},
		{
			path: 'appearance.sidebar.workspaces.rows',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: [['workspace', 'git_status'], ['location']],
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'sidebar',
			page: false,
		},
		{
			path: 'appearance.sidebar.workspaces.row_gap',
			type: 'integer',
			minimum: 0,
			maximum: 65535,
			step: 1,
			default: 0,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'appearance',
			group: 'sidebar',
			page: false,
		},
		{
			path: 'terminal.new_cwd',
			type: 'text',
			maximumBytes: 4096,
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'general',
			group: 'basics',
			page: true,
		},
		{
			path: 'terminal.focus_new_panel',
			type: 'boolean',
			default: true,
			scope: 'server',
			clients: 'both',
			effect: 'restart',
			category: 'terminal',
			group: 'panels',
			page: false,
		},
		{
			path: 'terminal.copy_on_select',
			type: 'boolean',
			default: false,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'terminal',
			group: 'input',
			page: true,
		},
		{
			path: 'terminal.macos_option_as_alt',
			type: 'choice',
			choices: ['off', 'left', 'right', 'both'],
			default: 'off',
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'terminal',
			group: 'input',
			page: true,
		},
		{
			path: 'terminal.scrollback_bytes',
			type: 'integer',
			minimum: 0,
			maximum: 1000000000,
			step: 1,
			default: 10000000,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'output',
			page: true,
		},
		{
			path: 'terminal.trim_trailing_blank_rows_for_agents',
			type: 'boolean',
			default: true,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'terminal',
			group: 'output',
			page: false,
		},
		{
			path: 'terminal.restore_output',
			type: 'boolean',
			default: false,
			scope: 'server',
			clients: 'both',
			effect: 'restart',
			category: 'general',
			group: 'restore',
			page: true,
		},
		{
			path: 'terminal.font_family',
			type: 'text',
			maximumBytes: 256,
			default: null,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: true,
		},
		{
			path: 'terminal.font_size',
			type: 'integer',
			minimum: 8,
			maximum: 48,
			step: 1,
			default: 13,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: true,
		},
		{
			path: 'terminal.font_weight',
			type: 'integer',
			minimum: 100,
			maximum: 900,
			step: 100,
			default: 400,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: false,
		},
		{
			path: 'terminal.font_weight_bold',
			type: 'integer',
			minimum: 100,
			maximum: 900,
			step: 100,
			default: 700,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: false,
		},
		{
			path: 'terminal.font_ligatures',
			type: 'boolean',
			default: true,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: false,
		},
		{
			path: 'terminal.padding',
			type: 'integer',
			minimum: 0,
			maximum: 32,
			step: 1,
			default: 0,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'text',
			page: false,
		},
		{
			path: 'terminal.background_opacity',
			type: 'integer',
			minimum: 10,
			maximum: 100,
			step: 1,
			default: 100,
			scope: 'client',
			clients: 'desktop',
			effect: 'immediate',
			category: 'appearance',
			group: 'tcolors',
			page: false,
		},
		{
			path: 'terminal.default_profile.<platform>',
			type: 'text',
			maximumBytes: 128,
			default: 'system-default',
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].id',
			type: 'text',
			maximumBytes: 128,
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].name',
			type: 'text',
			maximumBytes: 512,
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].kind',
			type: 'choice',
			choices: ['system-shell', 'custom-executable'],
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].login',
			type: 'boolean',
			default: false,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].executable',
			type: 'text',
			maximumBytes: 32768,
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'terminal.profiles.<platform>[].arguments',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: [],
			scope: 'server',
			clients: 'both',
			effect: 'new-terminals',
			category: 'terminal',
			group: 'profiles',
			page: true,
		},
		{
			path: 'worktrees.directory',
			type: 'path',
			default: null,
			scope: 'server',
			clients: 'both',
			effect: 'immediate',
			category: 'terminal',
			group: 'panels',
			page: false,
		},
		{
			path: 'notifications.agent_reminder',
			type: 'choice',
			choices: ['never', 'when-hidden'],
			default: 'never',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'when',
			page: true,
		},
		{
			path: 'notifications.completion_reminder',
			type: 'choice',
			choices: ['never', 'when-unfocused', 'always'],
			default: 'never',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'when',
			page: true,
		},
		{
			path: 'notifications.completion_minimum_seconds',
			type: 'integer',
			minimum: 1,
			maximum: 3600,
			step: 1,
			default: 5,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'when',
			page: false,
		},
		{
			path: 'notifications.sound',
			type: 'boolean',
			default: false,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'how',
			page: true,
		},
		{
			path: 'notifications.cli_delivery',
			type: 'choice',
			choices: ['off', 'orchbee', 'terminal', 'system'],
			default: 'off',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'how',
			page: false,
		},
		{
			path: 'notifications.delay_seconds',
			type: 'integer',
			minimum: 0,
			maximum: 3600,
			step: 1,
			default: 1,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'how',
			page: false,
		},
		{
			path: 'notifications.position',
			type: 'choice',
			choices: ['top-left', 'top-right', 'bottom-left', 'bottom-right'],
			default: 'bottom-right',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'how',
			page: false,
		},
		{
			path: 'notifications.sound_by_agent.*',
			type: 'choice',
			choices: ['default', 'on', 'off'],
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'notifications',
			group: 'how',
			page: false,
		},
		{
			path: 'keys.prefix',
			type: 'text',
			maximumBytes: 64,
			default: 'ctrl+b',
			scope: 'client',
			clients: 'tui',
			effect: 'immediate',
			category: 'keyboard',
			group: 'prefix',
			page: true,
		},
		{
			path: 'keys.<action>',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'tui',
			effect: 'immediate',
			category: 'keyboard',
			group: 'prefix',
			page: true,
		},
		{
			path: 'keys.command',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].command',
			type: 'text',
			maximumBytes: 4096,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].type',
			type: 'choice',
			choices: ['shell', 'panel', 'popup', 'plugin-action'],
			default: 'shell',
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].key',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].description',
			type: 'text',
			maximumBytes: 512,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].width',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'keys.command[].height',
			type: 'structured',
			maximumEntries: 64,
			maximumBytes: 65536,
			default: null,
			scope: 'client',
			clients: 'both',
			effect: 'immediate',
			category: 'keyboard',
			group: 'commands',
			page: false,
		},
		{
			path: 'agents.resume_on_restore',
			type: 'boolean',
			default: true,
			scope: 'server',
			clients: 'both',
			effect: 'restart',
			category: 'agents',
			group: 'restore',
			page: true,
		},
		{
			path: 'agent_detection.auto_update',
			type: 'boolean',
			default: true,
			scope: 'server',
			clients: 'both',
			effect: 'immediate',
			category: 'agents',
			group: 'updates',
			page: false,
		},
		{
			path: 'update.channel',
			type: 'choice',
			choices: ['stable', 'preview'],
			default: 'stable',
			scope: 'server',
			clients: 'both',
			effect: 'immediate',
			category: 'general',
			group: 'updates',
			page: false,
		},
	],
} as const;

type SettingKeyDefinition = (typeof SETTING_KEY_TABLE)['keys'][number];
export type SettingsPlatform = (typeof SETTING_KEY_TABLE)['platforms'][number];
type ScalarDefinition = Exclude<SettingKeyDefinition, { readonly type: 'structured' }>;
/** A path that names an open set of keys, such as one per Agent or per action, lists no value. */
type OpenPath = `${string}*${string}` | `${string}[]${string}` | `${string}<action>${string}`;
type ConcretePath<Path extends string> = Path extends OpenPath
	? never
	: Path extends `${infer Head}<platform>${infer Tail}`
		? `${Head}${SettingsPlatform}${Tail}`
		: Path;
type DefinitionValue<Definition> = Definition extends { readonly type: 'boolean' }
	? boolean
	: Definition extends { readonly type: 'integer' }
		? number
		: Definition extends { readonly type: 'choice'; readonly choices: readonly (infer Choice)[] }
			? Choice
			: string;
type DefinitionUnset<Definition> = Definition extends { readonly default: null } ? null : never;

/**
 * Every scalar key a settings snapshot lists, by concrete path, with the values it admits in the
 * file's spelling; null where the default is unset.
 */
export type SettingValues = {
	readonly [Definition in ScalarDefinition as ConcretePath<Definition['path']>]:
		| DefinitionValue<Definition>
		| DefinitionUnset<Definition>;
};
export type SettingKey = keyof SettingValues;

const MAXIMUM_PATH_BYTES = 4096;
const COLOR = /^#[\da-f]{6}$/iu;
const THEME_ID = /^[\da-z]+(?:-[\da-z]+)*$/u;
const utf8 = new TextEncoder();

const definitions = new Map<string, ScalarDefinition>();
for (const definition of SETTING_KEY_TABLE.keys) {
	if (definition.type === 'structured' || /\*|\[\]|<action>/u.test(definition.path)) continue;
	const paths = definition.path.includes('<platform>')
		? SETTING_KEY_TABLE.platforms.map((platform) => definition.path.replace('<platform>', platform))
		: [definition.path];
	for (const path of paths) definitions.set(path, definition);
}

function text(value: unknown, maximumBytes: number): value is string {
	return (
		typeof value === 'string' &&
		!value.includes('\0') &&
		utf8.encode(value).byteLength <= maximumBytes
	);
}

function isSettingKey(path: string): path is SettingKey {
	return definitions.has(path);
}

/** @internal Every scalar key, in table order, for the Web in-memory settings fixture. */
export const SETTING_KEYS: readonly SettingKey[] = [...definitions.keys()].filter(isSettingKey);

/**
 * Whether `value` is one that `key` admits by the table's type, bounds and choices. The Server's
 * registry still decides an edit; this keeps a Client from sending what it would refuse.
 */
function isSettingValue<Key extends SettingKey>(
	key: Key,
	value: unknown,
): value is SettingValues[Key] {
	const definition = definitions.get(key);
	if (definition === undefined) return false;
	if (value === null) return definition.default === null;
	switch (definition.type) {
		case 'boolean':
			return typeof value === 'boolean';
		case 'integer':
			return (
				typeof value === 'number' &&
				Number.isSafeInteger(value) &&
				value >= definition.minimum &&
				value <= definition.maximum &&
				(value - definition.minimum) % definition.step === 0
			);
		case 'choice':
			return definition.choices.some((choice) => choice === value);
		case 'color':
			return typeof value === 'string' && COLOR.test(value);
		case 'theme':
			return text(value, definition.maximumBytes) && THEME_ID.test(value);
		case 'text':
			return text(value, definition.maximumBytes);
		case 'path':
			return text(value, MAXIMUM_PATH_BYTES);
	}
}

/** The registry default of `key`; null where it is unset. */
export function settingDefault<Key extends SettingKey>(key: Key): SettingValues[Key] {
	const value = definitions.get(key)?.default ?? null;
	if (isSettingValue(key, value)) return value;
	throw new Error(`The setting key table gives ${key} no admissible default.`);
}

/** The value a snapshot lists for `key`, or its default when it lists none that the key admits. */
export function readSettingValue<Key extends SettingKey>(
	values: Readonly<Record<string, unknown>>,
	key: Key,
): SettingValues[Key] {
	const value = values[key];
	return isSettingValue(key, value) ? value : settingDefault(key);
}
