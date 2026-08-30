import { isAbsolute, join } from 'node:path';

import {
	SshEnvironmentTargetReferenceSchema,
	type SshEnvironmentTargetOption,
	type SshEnvironmentTargetReference,
} from './ssh-environment-contract';

const MAXIMUM_COMMAND_CHARACTERS = 1_024;
const MAXIMUM_COMMAND_TOKENS = 64;
const COMMAND_LINE_BREAKS = /[\r\n]/u;
const UNQUOTED_SHELL_CONTROL_CHARACTERS = /[`$;&|<>]/u;

interface OpenSshConnectionCommandParseOptions {
	readonly homeDirectory?: string | undefined;
}

export class OpenSshConnectionCommandParseError extends Error {}

export function parseOpenSshConnectionCommand(
	input: string,
	options: OpenSshConnectionCommandParseOptions = {},
): SshEnvironmentTargetReference {
	if (
		input.length === 0 ||
		input.length > MAXIMUM_COMMAND_CHARACTERS ||
		input !== input.trim() ||
		COMMAND_LINE_BREAKS.test(input)
	) {
		throw new OpenSshConnectionCommandParseError('Invalid SSH connection command.');
	}
	const tokens = tokenizeCommand(input);
	if (tokens.length > 1 && tokens[0] === 'ssh') tokens.shift();
	if (tokens.length === 0) {
		throw new OpenSshConnectionCommandParseError('Missing SSH destination.');
	}

	let configurationFilePath: null | string = null;
	let destination: string | undefined;
	const targetOptions: SshEnvironmentTargetOption[] = [];
	let optionsEnded = false;
	for (let index = 0; index < tokens.length; index += 1) {
		const token = tokens[index];
		if (token === undefined) break;
		if (!optionsEnded && token === '--') {
			optionsEnded = true;
			continue;
		}
		if (!optionsEnded && token.startsWith('-') && token !== '-') {
			const parsed = parseShortOptions(tokens, index, targetOptions, {
				setConfigurationFilePath: (value) => {
					if (configurationFilePath !== null) {
						throw new OpenSshConnectionCommandParseError(
							'SSH configuration file option cannot be repeated.',
						);
					}
					configurationFilePath = resolveConfigurationFilePath(value, options.homeDirectory);
				},
			});
			index = parsed.lastConsumedIndex;
			continue;
		}
		if (destination !== undefined) {
			throw new OpenSshConnectionCommandParseError('Remote SSH commands are not supported.');
		}
		destination = token;
	}

	const parsedTarget = SshEnvironmentTargetReferenceSchema.safeParse({
		configurationFilePath,
		destination,
		...(targetOptions.length === 0 ? {} : { options: targetOptions }),
	});
	if (!parsedTarget.success) {
		throw new OpenSshConnectionCommandParseError('Invalid SSH connection command.');
	}
	if (formatOpenSshConnectionCommand(parsedTarget.data).length > MAXIMUM_COMMAND_CHARACTERS) {
		throw new OpenSshConnectionCommandParseError('SSH connection command is too long.');
	}
	return parsedTarget.data;
}

export function formatOpenSshConnectionCommand(target: SshEnvironmentTargetReference): string {
	const canonical = SshEnvironmentTargetReferenceSchema.parse(target);
	const targetOptions = canonical.options ?? [];
	if (canonical.configurationFilePath === null && targetOptions.length === 0) {
		return quoteCommandToken(canonical.destination);
	}
	const tokens = ['ssh'];
	if (canonical.configurationFilePath !== null) {
		tokens.push('-F', canonical.configurationFilePath);
	}
	for (const option of targetOptions) tokens.push(...formatTargetOption(option));
	tokens.push(canonical.destination);
	return tokens.map(quoteCommandToken).join(' ');
}

export function createOpenSshTargetOptionArguments(
	options: readonly SshEnvironmentTargetOption[] | undefined,
): string[] {
	return (options ?? []).flatMap((option) => {
		switch (option.kind) {
			case 'address-family':
				return [option.value === 'inet' ? '-4' : '-6'];
			case 'compression':
				return ['-o', `Compression=${option.enabled ? 'yes' : 'no'}`];
			case 'forward-agent':
				return ['-o', `ForwardAgent=${option.enabled ? 'yes' : 'no'}`];
			case 'identity-file':
				return ['-i', option.value];
			case 'port':
				return ['-p', String(option.value)];
			case 'proxy-jump':
				return ['-J', option.value];
			case 'user':
				return ['-l', option.value];
		}
	});
}

function parseShortOptions(
	tokens: readonly string[],
	startIndex: number,
	targetOptions: SshEnvironmentTargetOption[],
	context: {
		readonly setConfigurationFilePath: (value: string) => void;
	},
): { readonly lastConsumedIndex: number } {
	const token = tokens[startIndex];
	if (token === undefined) {
		throw new OpenSshConnectionCommandParseError('Missing SSH option.');
	}
	for (let offset = 1; offset < token.length; offset += 1) {
		const option = token[offset];
		if (option === undefined) break;
		switch (option) {
			case '4':
				addSingletonOption(targetOptions, { kind: 'address-family', value: 'inet' });
				continue;
			case '6':
				addSingletonOption(targetOptions, { kind: 'address-family', value: 'inet6' });
				continue;
			case 'A':
				addSingletonOption(targetOptions, { enabled: true, kind: 'forward-agent' });
				continue;
			case 'C':
				addSingletonOption(targetOptions, { enabled: true, kind: 'compression' });
				continue;
			case 'a':
				addSingletonOption(targetOptions, { enabled: false, kind: 'forward-agent' });
				continue;
			case 'F':
			case 'J':
			case 'i':
			case 'l':
			case 'o':
			case 'p': {
				const inlineValue = token.slice(offset + 1);
				const value = inlineValue.length > 0 ? inlineValue : tokens[startIndex + 1];
				if (value === undefined || value.length === 0) {
					throw new OpenSshConnectionCommandParseError(`SSH option -${option} requires a value.`);
				}
				applyValueOption(option, value, targetOptions, context);
				return {
					lastConsumedIndex: inlineValue.length > 0 ? startIndex : startIndex + 1,
				};
			}
			default:
				throw new OpenSshConnectionCommandParseError(`Unsupported SSH option -${option}.`);
		}
	}
	return { lastConsumedIndex: startIndex };
}

function applyValueOption(
	option: 'F' | 'J' | 'i' | 'l' | 'o' | 'p',
	value: string,
	targetOptions: SshEnvironmentTargetOption[],
	context: {
		readonly setConfigurationFilePath: (value: string) => void;
	},
): void {
	switch (option) {
		case 'F':
			context.setConfigurationFilePath(value);
			return;
		case 'J':
			addSingletonOption(targetOptions, { kind: 'proxy-jump', value });
			return;
		case 'i':
			targetOptions.push({ kind: 'identity-file', value });
			return;
		case 'l':
			addSingletonOption(targetOptions, { kind: 'user', value });
			return;
		case 'o':
			applyOpenSshSetting(value, targetOptions);
			return;
		case 'p':
			addPortOption(value, targetOptions);
	}
}

function applyOpenSshSetting(setting: string, targetOptions: SshEnvironmentTargetOption[]): void {
	const separator = setting.search(/[=\s]/u);
	if (separator <= 0) {
		throw new OpenSshConnectionCommandParseError(
			'SSH -o options must use Name=Value or Name Value syntax.',
		);
	}
	const name = setting.slice(0, separator).toLowerCase();
	const value = setting.slice(separator + 1).trim();
	if (value.length === 0) {
		throw new OpenSshConnectionCommandParseError('SSH -o option value is missing.');
	}
	switch (name) {
		case 'addressfamily': {
			const family = value.toLowerCase();
			if (family !== 'inet' && family !== 'inet6') {
				throw new OpenSshConnectionCommandParseError('Unsupported AddressFamily value.');
			}
			addSingletonOption(targetOptions, { kind: 'address-family', value: family });
			return;
		}
		case 'compression':
			addSingletonOption(targetOptions, {
				enabled: parseYesNo(value, 'Compression'),
				kind: 'compression',
			});
			return;
		case 'forwardagent':
			addSingletonOption(targetOptions, {
				enabled: parseYesNo(value, 'ForwardAgent'),
				kind: 'forward-agent',
			});
			return;
		case 'identityfile':
			targetOptions.push({ kind: 'identity-file', value });
			return;
		case 'port':
			addPortOption(value, targetOptions);
			return;
		case 'proxyjump':
			addSingletonOption(targetOptions, { kind: 'proxy-jump', value });
			return;
		case 'user':
			addSingletonOption(targetOptions, { kind: 'user', value });
			return;
		default:
			throw new OpenSshConnectionCommandParseError(`Unsupported SSH -o option ${name}.`);
	}
}

function addPortOption(value: string, targetOptions: SshEnvironmentTargetOption[]): void {
	if (!/^\d{1,5}$/u.test(value)) {
		throw new OpenSshConnectionCommandParseError('Invalid SSH port.');
	}
	const port = Number(value);
	if (port < 1 || port > 65_535) {
		throw new OpenSshConnectionCommandParseError('Invalid SSH port.');
	}
	addSingletonOption(targetOptions, { kind: 'port', value: port });
}

function addSingletonOption(
	options: SshEnvironmentTargetOption[],
	option: Exclude<SshEnvironmentTargetOption, { readonly kind: 'identity-file' }>,
): void {
	if (options.some((candidate) => candidate.kind === option.kind)) {
		throw new OpenSshConnectionCommandParseError(`SSH option ${option.kind} cannot be repeated.`);
	}
	options.push(option);
}

function parseYesNo(value: string, name: string): boolean {
	switch (value.toLowerCase()) {
		case 'yes':
		case 'true':
			return true;
		case 'no':
		case 'false':
			return false;
		default:
			throw new OpenSshConnectionCommandParseError(`Invalid ${name} value.`);
	}
}

function resolveConfigurationFilePath(value: string, homeDirectory: string | undefined): string {
	const expanded =
		homeDirectory !== undefined && (value === '~' || value.startsWith('~/'))
			? value === '~'
				? homeDirectory
				: join(homeDirectory, value.slice(2))
			: value;
	if (!isAbsolute(expanded)) {
		throw new OpenSshConnectionCommandParseError('SSH configuration file path must be absolute.');
	}
	return expanded;
}

function tokenizeCommand(input: string): string[] {
	const tokens: string[] = [];
	let current = '';
	let currentStarted = false;
	let quote: 'double' | 'single' | null = null;
	for (let index = 0; index < input.length; index += 1) {
		const character = input[index];
		if (character === undefined) break;
		if (quote === 'single') {
			if (character === "'") quote = null;
			else current += character;
			continue;
		}
		if (quote === 'double') {
			if (character === '"') {
				quote = null;
			} else if (character === '\\') {
				index += 1;
				const escaped = input[index];
				if (escaped === undefined) {
					throw new OpenSshConnectionCommandParseError('Incomplete quoted SSH option.');
				}
				current += escaped;
			} else {
				current += character;
			}
			continue;
		}
		if (UNQUOTED_SHELL_CONTROL_CHARACTERS.test(character)) {
			throw new OpenSshConnectionCommandParseError('Unquoted shell syntax is not supported.');
		}
		if (/\s/u.test(character)) {
			if (currentStarted) {
				tokens.push(current);
				if (tokens.length > MAXIMUM_COMMAND_TOKENS) {
					throw new OpenSshConnectionCommandParseError('SSH command has too many options.');
				}
				current = '';
				currentStarted = false;
			}
			continue;
		}
		currentStarted = true;
		if (character === "'") {
			quote = 'single';
		} else if (character === '"') {
			quote = 'double';
		} else if (character === '\\') {
			index += 1;
			const escaped = input[index];
			if (escaped === undefined) {
				throw new OpenSshConnectionCommandParseError('Incomplete SSH option escape.');
			}
			current += escaped;
		} else {
			current += character;
		}
	}
	if (quote !== null) {
		throw new OpenSshConnectionCommandParseError('Unclosed SSH option quote.');
	}
	if (currentStarted) tokens.push(current);
	if (tokens.length > MAXIMUM_COMMAND_TOKENS) {
		throw new OpenSshConnectionCommandParseError('SSH command has too many options.');
	}
	return tokens;
}

function formatTargetOption(option: SshEnvironmentTargetOption): string[] {
	switch (option.kind) {
		case 'address-family':
			return [option.value === 'inet' ? '-4' : '-6'];
		case 'compression':
			return option.enabled ? ['-C'] : ['-o', 'Compression=no'];
		case 'forward-agent':
			return [option.enabled ? '-A' : '-a'];
		case 'identity-file':
			return ['-i', option.value];
		case 'port':
			return ['-p', String(option.value)];
		case 'proxy-jump':
			return ['-J', option.value];
		case 'user':
			return ['-l', option.value];
	}
}

function quoteCommandToken(token: string): string {
	return /^[A-Za-z0-9_@%+=:,./~-]+$/u.test(token) ? token : `'${token.replaceAll("'", `'\\''`)}'`;
}
