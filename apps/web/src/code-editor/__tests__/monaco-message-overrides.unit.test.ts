import { readdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

import { zhCNMonacoMessageOverrides } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

/**
 * The English that the pinned `monaco-editor` passes with each overridden message index. Monaco
 * renumbers its messages between versions, so an index that now carries other English means its
 * override must move to the index that carries this English.
 */
const PINNED_ENGLISH: Readonly<Record<string, string>> = {
	145: 'Collapse unchanged lines',
	146: 'Show {0} hidden lines',
	149: 'Collapse unchanged lines',
	150: 'Go to {0}',
	153: 'Code moved to line {0}-{1}',
	1014: 'Go to Match...',
	1198: "Rendering paused for long line for performance reasons. This can be configured via `editor.stopRenderingLineAfter`. [Don't Show Again](command:{0})",
	1364: 'Cursors added: {0}',
	1529: 'Focus Editor Sticky Scroll',
	1530: 'Select the next editor sticky scroll line',
	1531: 'Select the previous sticky scroll line',
	1532: 'Go to the focused sticky scroll line',
	1627: 'Disable Highlight In Comments',
	1635: 'Disable highlighting of characters in comments',
};

const zhCNTablePath = createRequire(import.meta.url).resolve('monaco-editor/nls/lang/zh-cn');
const monacoSourceRoot = dirname(dirname(dirname(zhCNTablePath)));

/** The pinned zh-CN table as Monaco installs it: one entry per message index, `null` where untranslated. */
function pinnedZhCNTable(): readonly unknown[] {
	const text = readFileSync(zhCNTablePath, 'utf8');
	const assignment = '_VSCODE_NLS_MESSAGES=';
	const table: unknown = JSON.parse(
		text.slice(text.indexOf(assignment) + assignment.length, text.lastIndexOf(']') + 1),
	);
	if (!Array.isArray(table)) throw new TypeError('The pinned zh-CN table is not an array.');
	return table;
}

/** The English that the built `localize` or `localize2` call site of each of `indices` passes. */
function pinnedEnglish(indices: ReadonlySet<number>): ReadonlyMap<number, string> {
	const sources = new Map<number, string>();
	const callSite = /\blocalize2?\(\s*(\d+)\s*,\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')/gu;
	const visit = (directory: string): void => {
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) {
				visit(path);
				continue;
			}
			if (!entry.name.endsWith('.js')) continue;
			for (const [, index, literal] of readFileSync(path, 'utf8').matchAll(callSite)) {
				if (index === undefined || literal === undefined || !indices.has(Number(index))) continue;
				// Either quote style, rewritten as the JSON string it denotes.
				const body = literal
					.slice(1, -1)
					.replaceAll(String.raw`\'`, "'")
					.replaceAll(String.raw`\"`, '"')
					.replaceAll('"', String.raw`\"`);
				const english: unknown = JSON.parse(`"${body}"`);
				if (typeof english === 'string') sources.set(Number(index), english);
			}
		}
	};
	visit(monacoSourceRoot);
	return sources;
}

function placeholders(message: string): readonly string[] {
	return [...message.matchAll(/\{\d+\}/gu)]
		.map(([placeholder]) => placeholder)
		.sort((first, second) => first.localeCompare(second));
}

describe('the zh-CN Monaco message overrides', () => {
	it('replace pinned messages whose English is still the one they translate', () => {
		const overrides = Object.entries(zhCNMonacoMessageOverrides);
		const table = pinnedZhCNTable();
		const english = pinnedEnglish(new Set(overrides.map(([index]) => Number(index))));

		expect(overrides.map(([index]) => index)).toEqual(Object.keys(PINNED_ENGLISH));
		for (const [index, override] of overrides) {
			const anchor = PINNED_ENGLISH[index] ?? '';
			expect(Number(index), `override ${index} is outside the pinned table`).toBeLessThan(
				table.length,
			);
			expect(english.get(Number(index)), `override ${index} translates other English`).toBe(anchor);
			expect(placeholders(override), `override ${index} changes placeholders`).toEqual(
				placeholders(anchor),
			);
		}
	});
});
