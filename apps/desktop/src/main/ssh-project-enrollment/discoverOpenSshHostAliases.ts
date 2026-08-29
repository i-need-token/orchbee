import { glob, open, realpath } from 'node:fs/promises';
import path from 'node:path';

import type { DesktopSshEnvironmentDiscoveryIssue } from '@orchbee/desktop-client-protocol';

const OPEN_SSH_HOST_DISCOVERY_LIMITS = Object.freeze({
	maximumAliasCount: 1_024,
	maximumConfigurationBytes: 1024 * 1024,
	maximumFileCount: 64,
	maximumIncludeCount: 256,
});

export interface OpenSshHostAliasDiscovery {
	readonly aliases: readonly string[];
	readonly issue: DesktopSshEnvironmentDiscoveryIssue | null;
}

interface OpenSshHostDiscoveryContext {
	aliasKeys: Set<string>;
	aliases: string[];
	configurationBytes: number;
	configurationDirectory: string;
	environment: Readonly<NodeJS.ProcessEnv>;
	fileCount: number;
	homeDirectory: string;
	includeCount: number;
	issue: DesktopSshEnvironmentDiscoveryIssue | null;
	visitedFiles: Set<string>;
}

type ConfigurationReadResult =
	| { readonly kind: 'missing' }
	| { readonly kind: 'ready'; readonly source: string; readonly byteLength: number }
	| { readonly issue: DesktopSshEnvironmentDiscoveryIssue; readonly kind: 'unavailable' };

interface SshConfigurationDirective {
	readonly arguments: readonly string[];
	readonly keyword: string;
}

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export async function discoverOpenSshHostAliases(
	configurationFilePath: string,
	homeDirectory: string,
	environment: Readonly<NodeJS.ProcessEnv> = process.env,
): Promise<OpenSshHostAliasDiscovery> {
	const context: OpenSshHostDiscoveryContext = {
		aliasKeys: new Set(),
		aliases: [],
		configurationBytes: 0,
		configurationDirectory: path.join(path.resolve(homeDirectory), '.ssh'),
		environment,
		fileCount: 0,
		homeDirectory: path.resolve(homeDirectory),
		includeCount: 0,
		issue: null,
		visitedFiles: new Set(),
	};
	await inspectConfiguration(path.resolve(configurationFilePath), context, true);
	return {
		aliases: [...context.aliases].sort(compareAliases),
		issue: context.issue,
	};
}

async function inspectConfiguration(
	configurationFilePath: string,
	context: OpenSshHostDiscoveryContext,
	root: boolean,
): Promise<void> {
	if (context.fileCount >= OPEN_SSH_HOST_DISCOVERY_LIMITS.maximumFileCount) {
		context.issue = mergeIssue(context.issue, 'truncated');
		return;
	}
	let canonicalPath: string;
	try {
		canonicalPath = await realpath(configurationFilePath);
	} catch (error: unknown) {
		if (isMissing(error)) {
			if (root) return;
			return;
		}
		context.issue = mergeIssue(context.issue, 'read-failed');
		return;
	}
	if (context.visitedFiles.has(canonicalPath)) return;
	context.visitedFiles.add(canonicalPath);
	context.fileCount += 1;
	const remainingBytes =
		OPEN_SSH_HOST_DISCOVERY_LIMITS.maximumConfigurationBytes - context.configurationBytes;
	const read = await readConfiguration(canonicalPath, remainingBytes);
	if (read.kind === 'missing') return;
	if (read.kind === 'unavailable') {
		context.issue = mergeIssue(context.issue, read.issue);
		return;
	}
	context.configurationBytes += read.byteLength;
	for (const line of read.source.split(/\r?\n/u)) {
		const directive = parseDirective(line);
		if (directive === 'invalid') {
			context.issue = mergeIssue(context.issue, 'invalid-config');
			continue;
		}
		if (directive === null) continue;
		if (directive.keyword === 'host') {
			for (const candidate of directive.arguments) admitAlias(candidate, context);
			continue;
		}
		if (directive.keyword !== 'include') continue;
		for (const include of directive.arguments) {
			if (context.includeCount >= OPEN_SSH_HOST_DISCOVERY_LIMITS.maximumIncludeCount) {
				context.issue = mergeIssue(context.issue, 'truncated');
				break;
			}
			context.includeCount += 1;
			const expanded = expandInclude(include, context);
			if (expanded === null) {
				context.issue = mergeIssue(context.issue, 'invalid-config');
				continue;
			}
			let matches: string[];
			try {
				matches = [];
				for await (const match of glob(expanded)) {
					matches.push(path.resolve(match));
					if (matches.length > OPEN_SSH_HOST_DISCOVERY_LIMITS.maximumFileCount) {
						context.issue = mergeIssue(context.issue, 'truncated');
						break;
					}
				}
			} catch {
				context.issue = mergeIssue(context.issue, 'read-failed');
				continue;
			}
			for (const match of matches.sort()) await inspectConfiguration(match, context, false);
		}
	}
}

async function readConfiguration(
	configurationFilePath: string,
	remainingBytes: number,
): Promise<ConfigurationReadResult> {
	let handle: Awaited<ReturnType<typeof open>>;
	try {
		handle = await open(configurationFilePath, 'r');
	} catch (error: unknown) {
		return isMissing(error) ? { kind: 'missing' } : { issue: 'read-failed', kind: 'unavailable' };
	}
	try {
		const metadata = await handle.stat();
		if (!metadata.isFile()) return { issue: 'invalid-config', kind: 'unavailable' };
		if (metadata.size > remainingBytes) {
			return { issue: 'truncated', kind: 'unavailable' };
		}
		const encoded = await handle.readFile();
		if (encoded.byteLength > remainingBytes) {
			return { issue: 'truncated', kind: 'unavailable' };
		}
		try {
			return {
				byteLength: encoded.byteLength,
				kind: 'ready',
				source: utf8Decoder.decode(encoded),
			};
		} catch {
			return { issue: 'invalid-config', kind: 'unavailable' };
		}
	} catch {
		return { issue: 'read-failed', kind: 'unavailable' };
	} finally {
		await handle.close().catch(() => undefined);
	}
}

function parseDirective(line: string): SshConfigurationDirective | 'invalid' | null {
	const tokens = tokenize(line);
	if (tokens === 'invalid' || tokens.length === 0) return tokens === 'invalid' ? tokens : null;
	const [head, ...tail] = tokens;
	if (head === undefined) return null;
	const equals = head.indexOf('=');
	if (equals === 0) return 'invalid';
	if (equals > 0) {
		const inlineArgument = head.slice(equals + 1);
		return {
			arguments: inlineArgument.length === 0 ? tail : [inlineArgument, ...tail],
			keyword: head.slice(0, equals).toLowerCase(),
		};
	}
	return { arguments: tail, keyword: head.toLowerCase() };
}

function tokenize(line: string): readonly string[] | 'invalid' {
	const tokens: string[] = [];
	let current = '';
	let escaped = false;
	let quote: 'double' | 'single' | null = null;
	const complete = (): void => {
		if (current.length === 0) return;
		tokens.push(current);
		current = '';
	};
	for (const character of line) {
		if (escaped) {
			current += character;
			escaped = false;
			continue;
		}
		if (character === '\\' && quote !== 'single') {
			escaped = true;
			continue;
		}
		if (character === "'" && quote !== 'double') {
			quote = quote === 'single' ? null : 'single';
			continue;
		}
		if (character === '"' && quote !== 'single') {
			quote = quote === 'double' ? null : 'double';
			continue;
		}
		if (character === '#' && quote === null) break;
		if (/\s/u.test(character) && quote === null) {
			complete();
			continue;
		}
		current += character;
	}
	if (escaped || quote !== null) return 'invalid';
	complete();
	return tokens;
}

function admitAlias(candidate: string, context: OpenSshHostDiscoveryContext): void {
	if (!isConcreteAlias(candidate)) return;
	const key = candidate.toLowerCase();
	if (context.aliasKeys.has(key)) return;
	if (context.aliases.length >= OPEN_SSH_HOST_DISCOVERY_LIMITS.maximumAliasCount) {
		context.issue = mergeIssue(context.issue, 'truncated');
		return;
	}
	context.aliasKeys.add(key);
	context.aliases.push(candidate);
}

function isConcreteAlias(value: string): boolean {
	return (
		value.length > 0 &&
		value.length <= 1_024 &&
		!value.startsWith('-') &&
		!/[\s*?![\]]/u.test(value) &&
		hasNoControlCharacters(value)
	);
}

function expandInclude(value: string, context: OpenSshHostDiscoveryContext): string | null {
	let expanded = value.replace(/%%/gu, '\0');
	expanded = expanded.replace(/%d/gu, context.homeDirectory);
	if (/%[A-Za-z]/u.test(expanded)) return null;
	expanded = expanded.replaceAll('\0', '%');
	let missingEnvironment = false;
	expanded = expanded.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}/gu, (_match, name: string) => {
		const replacement = context.environment[name];
		if (replacement === undefined) {
			missingEnvironment = true;
			return '';
		}
		return replacement;
	});
	if (missingEnvironment || expanded.length === 0 || expanded.includes('\0')) return null;
	if (expanded === '~') expanded = context.homeDirectory;
	else if (expanded.startsWith('~/'))
		expanded = path.join(context.homeDirectory, expanded.slice(2));
	else if (expanded.startsWith('~')) return null;
	return path.isAbsolute(expanded)
		? path.normalize(expanded)
		: path.resolve(context.configurationDirectory, expanded);
}

function mergeIssue(
	current: DesktopSshEnvironmentDiscoveryIssue | null,
	next: DesktopSshEnvironmentDiscoveryIssue,
): DesktopSshEnvironmentDiscoveryIssue {
	const severity: Readonly<Record<DesktopSshEnvironmentDiscoveryIssue, number>> = {
		'invalid-config': 1,
		'read-failed': 2,
		truncated: 3,
	};
	return current === null || severity[next] > severity[current] ? next : current;
}

function compareAliases(left: string, right: string): number {
	const canonicalOrder = left.toLowerCase().localeCompare(right.toLowerCase(), 'en-US');
	return canonicalOrder === 0 ? left.localeCompare(right, 'en-US') : canonicalOrder;
}

function hasNoControlCharacters(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f)) return false;
	}
	return true;
}

function isMissing(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}
