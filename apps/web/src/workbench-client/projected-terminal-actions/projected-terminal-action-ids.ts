import type { ActionInputValidation } from '@orchbee/actions';
import {
	TERMINAL_ACTION_IDS,
	type WorkbenchTerminalActionCatalog,
	type WorkbenchTerminalLinkActionInput,
	type WorkbenchTerminalPathsInput,
} from '../../workbench';

import { PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS } from '../../projected-terminal-keyboard-shortcuts';

export const PROJECTED_TERMINAL_ACTION_IDS = {
	clear: TERMINAL_ACTION_IDS.clear,
	copySelection: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.copySelection,
	findNext: TERMINAL_ACTION_IDS.findNext,
	findOpen: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.findOpen,
	findPrevious: TERMINAL_ACTION_IDS.findPrevious,
	hintsOpen: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.hintsOpen,
	insertPaths: TERMINAL_ACTION_IDS.insertPaths,
	openLink: TERMINAL_ACTION_IDS.openLink,
	paste: TERMINAL_ACTION_IDS.paste,
	promptNext: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.promptNext,
	promptPrevious: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.promptPrevious,
	reset: TERMINAL_ACTION_IDS.reset,
	scrollLineDown: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollLineDown,
	scrollLineUp: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollLineUp,
	scrollPageDown: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollPageDown,
	scrollPageUp: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollPageUp,
	scrollToBottom: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollToBottom,
	scrollToTop: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollToTop,
	selectAll: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.selectAll,
	takeControl: TERMINAL_ACTION_IDS.takeControl,
} as const;

export type ProjectedTerminalLinkActionInput = WorkbenchTerminalLinkActionInput;
export type ProjectedTerminalActionCatalog = WorkbenchTerminalActionCatalog;

export const projectedTerminalLinkActionInput: ActionInputValidation<ProjectedTerminalLinkActionInput> =
	{
		name: 'terminal-link-target',
		parse(input) {
			if (!isRecord(input) || !isRecord(input['target'])) {
				return { code: 'invalid-terminal-link-target', success: false };
			}
			const context = optionalLinkContext(input['context']);
			if (context === false) return { code: 'invalid-terminal-link-target', success: false };
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
							...(context === undefined ? {} : { context }),
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
						: {
								data: {
									...(context === undefined ? {} : { context }),
									target: { kind: 'unknown', value },
								},
								success: true,
							};
				}
				case 'url': {
					const url = boundedString(target['url']);
					return url === null
						? { code: 'invalid-terminal-link-target', success: false }
						: {
								data: {
									...(context === undefined ? {} : { context }),
									target: { kind: 'url', url },
								},
								success: true,
							};
				}
				default:
					return { code: 'invalid-terminal-link-target', success: false };
			}
		},
	};

/** At least one path, each a nonempty string of at most 16,384 characters, for a POSIX or Windows shell. */
export const projectedTerminalPathsActionInput: ActionInputValidation<WorkbenchTerminalPathsInput> =
	{
		name: 'terminal-paths',
		parse(input) {
			if (!isRecord(input)) return { code: 'invalid-terminal-paths', success: false };
			const { paths, platform } = input;
			return Array.isArray(paths) &&
				paths.length > 0 &&
				paths.every((path): path is string => boundedString(path) !== null) &&
				(platform === 'posix' || platform === 'windows')
				? { data: { paths: [...paths], platform }, success: true }
				: { code: 'invalid-terminal-paths', success: false };
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

function optionalLinkContext(
	value: unknown,
): false | NonNullable<ProjectedTerminalLinkActionInput['context']> | undefined {
	if (value === undefined) return undefined;
	if (!isRecord(value)) return false;
	const column = value['column'];
	const generation = canonicalUnsignedText(value['generation']);
	const linkEpoch = canonicalUnsignedText(value['linkEpoch']);
	const revision = canonicalUnsignedText(value['revision']);
	const row = canonicalUnsignedText(value['row']);
	if (
		typeof column !== 'number' ||
		!Number.isSafeInteger(column) ||
		column < 0 ||
		generation === null ||
		linkEpoch === null ||
		revision === null ||
		row === null
	) {
		return false;
	}
	return { column, generation, linkEpoch, revision, row };
}

function canonicalUnsignedText(value: unknown): string | null {
	return typeof value === 'string' && /^(?:0|[1-9][0-9]*)$/u.test(value) ? value : null;
}
