import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';
import type { TerminalOpenTarget } from '@orchbee/terminal';

export const DESKTOP_TERMINAL_ACTION_IDS = {
	copySelection: 'terminal.selection.copy',
	findNext: 'terminal.search.next',
	findOpen: 'terminal.search.open',
	findPrevious: 'terminal.search.previous',
	openLink: 'terminal.link.open',
	selectAll: 'terminal.selection.select-all',
} as const;

export interface DesktopTerminalLinkActionInput {
	readonly target: TerminalOpenTarget;
}

export interface DesktopTerminalActionCatalog {
	readonly 'terminal.link.open': ActionSignature<DesktopTerminalLinkActionInput>;
	readonly 'terminal.search.next': ActionSignature<void>;
	readonly 'terminal.search.open': ActionSignature<void>;
	readonly 'terminal.search.previous': ActionSignature<void>;
	readonly 'terminal.selection.copy': ActionSignature<void>;
	readonly 'terminal.selection.select-all': ActionSignature<void>;
}

export const desktopTerminalLinkActionInput: ActionInputValidation<DesktopTerminalLinkActionInput> =
	{
		name: 'terminal-link-target',
		parse(input) {
			if (!isRecord(input) || !isRecord(input['target'])) {
				return { code: 'invalid-terminal-link-target', success: false };
			}
			const target = input['target'];
			switch (target['kind']) {
				case 'file': {
					const path = boundedString(target['path']);
					if (path === null) return { code: 'invalid-terminal-link-target', success: false };
					const line = optionalPositiveInteger(target['line']);
					const column = optionalPositiveInteger(target['column']);
					if (line === false || column === false) {
						return { code: 'invalid-terminal-link-target', success: false };
					}
					return {
						data: {
							target: {
								kind: 'file',
								path,
								...(typeof line === 'number' ? { line } : {}),
								...(typeof column === 'number' ? { column } : {}),
							},
						},
						success: true,
					};
				}
				case 'unknown': {
					const value = boundedString(target['value']);
					return value === null
						? { code: 'invalid-terminal-link-target', success: false }
						: { data: { target: { kind: 'unknown', value } }, success: true };
				}
				case 'url': {
					const url = boundedString(target['url']);
					return url === null
						? { code: 'invalid-terminal-link-target', success: false }
						: { data: { target: { kind: 'url', url } }, success: true };
				}
				default:
					return { code: 'invalid-terminal-link-target', success: false };
			}
		},
	};

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function boundedString(value: unknown): string | null {
	return typeof value === 'string' && value.length > 0 && value.length <= 16_384 ? value : null;
}

function optionalPositiveInteger(value: unknown): number | false | undefined {
	if (value === undefined) return undefined;
	return Number.isInteger(value) && typeof value === 'number' && value >= 1 ? value : false;
}
