import { describe, expect, it } from 'vitest';

import { scmRowMenus, type ScmMenuTarget, type ScmRowCommand } from '../scm-menus';
import fixture from './fixtures/vscode-scm-menus.json';

// The fixture holds the pinned VS Code's Git row menus verbatim. Each test evaluates their `when`
// clauses for one row with VS Code's default settings and sorts them by VS Code's menu rule, so a
// command the model adds, drops or moves shows here.

const COMMANDS: Readonly<Record<string, ScmRowCommand>> = {
	'git.clean': 'clean',
	'git.cleanAll': 'clean-all',
	'git.close': 'close',
	'git.closeOtherRepositories': 'close-others',
	'git.ignore': 'ignore',
	'git.openChange': 'open-change',
	'git.openFile': 'open-file',
	'git.openFile2': 'open-file',
	'git.openHEADFile': 'open-head-file',
	'git.revealFileInOS.linux': 'reveal-in-os',
	'git.revealFileInOS.mac': 'reveal-in-os',
	'git.revealFileInOS.windows': 'reveal-in-os',
	'git.revealInExplorer': 'reveal-in-explorer',
	'git.stage': 'stage',
	'git.stageAll': 'stage-all',
	'git.unstage': 'unstage',
	'git.unstageAll': 'unstage-all',
	'git.viewChanges': 'view-changes',
	'git.viewStagedChanges': 'view-staged-changes',
	'workbench.scm.action.collapseAll': 'collapse-all',
};

type MenuKey = keyof typeof fixture.menus;
type Context = Readonly<Record<string, boolean | number | string>>;

/** The VS Code settings the menus read, at their defaults. */
const SETTINGS: Context = {
	'config.git.openDiffOnClick': true,
	'config.git.showInlineOpenFileAction': true,
	'config.git.untrackedChanges': 'mixed',
	scmProvider: 'git',
};

/** VS Code's `when` clauses as the Git menus write them: terms joined by `&&`. */
function holds(when: string, context: Context): boolean {
	if (when === '') return true;
	return when.split('&&').every((raw) => {
		const term = raw.trim();
		const comparison = /^(\S+)\s*(==|!=|>)\s*(.+)$/u.exec(term);
		if (comparison === null) {
			return term.startsWith('!') ? !context[term.slice(1)] : Boolean(context[term]);
		}
		const [, key = '', operator, operand = ''] = comparison;
		const actual = context[key];
		const expected = operand.replaceAll("'", '');
		if (operator === '>') return Number(actual ?? 0) > Number(expected);
		const equal = String(actual ?? '') === expected;
		return operator === '==' ? equal : !equal;
	});
}

interface SortedEntry {
	readonly command: string;
	readonly group: string;
	readonly order: number;
}

/** VS Code's `compareMenuItems`: navigation first, groups by name, then order, then title. */
function compareEntries(one: SortedEntry, other: SortedEntry): number {
	if (one.group !== other.group) {
		if (one.group === 'navigation') return -1;
		if (other.group === 'navigation') return 1;
		return one.group.localeCompare(other.group);
	}
	if (one.order !== other.order) return one.order - other.order;
	const titles: Readonly<Record<string, string>> = fixture.titles;
	return (titles[one.command] ?? '').localeCompare(titles[other.command] ?? '');
}

function vscodeMenus(key: MenuKey, context: Context) {
	const entries = fixture.menus[key]
		.filter((entry) => holds(entry.when, { ...SETTINGS, ...context }))
		.map(({ command, group }) => {
			const [name = '', order] = group.split('@');
			return { command, group: name, order: Number(order ?? 0) };
		})
		.sort(compareEntries);
	const commandOf = ({ command }: SortedEntry): ScmRowCommand => {
		const mapped = COMMANDS[command];
		if (mapped === undefined) throw new Error(`No row command stands for ${command}.`);
		return mapped;
	};
	const context_: ScmRowCommand[][] = [];
	let group: string | null = null;
	for (const entry of entries.filter(({ group: name }) => name !== 'inline')) {
		if (entry.group !== group) context_.push([]);
		group = entry.group;
		context_.at(-1)?.push(commandOf(entry));
	}
	return {
		context: context_,
		inline: entries.filter(({ group: name }) => name === 'inline').map(commandOf),
	};
}

const PLATFORMS = [
	{ isLinux: true, isMac: false, isWindows: false },
	{ isLinux: false, isMac: true, isWindows: false },
	{ isLinux: false, isMac: false, isWindows: true },
];

describe('scmRowMenus', () => {
	it.each(['index', 'workingTree'] as const)("matches VS Code's file menus in %s", (group) => {
		for (const platform of PLATFORMS) {
			for (const local of [true, false]) {
				const target: ScmMenuTarget = { group, kind: 'resource' };
				const context = {
					...platform,
					remoteName: local ? '' : 'ssh-remote',
					scmResourceGroup: group,
				};

				expect(scmRowMenus(target, local)).toEqual(
					vscodeMenus('scm/resourceState/context', context),
				);
			}
		}
	});

	it.each(['index', 'workingTree'] as const)("matches VS Code's folder menus in %s", (group) => {
		expect(scmRowMenus({ group, kind: 'folder' }, true)).toEqual(
			vscodeMenus('scm/resourceFolder/context', { scmResourceGroup: group }),
		);
	});

	it.each(['index', 'workingTree'] as const)("matches VS Code's group menus in %s", (group) => {
		for (const mode of ['list', 'tree'] as const) {
			expect(scmRowMenus({ group, kind: 'group', mode }, true)).toEqual(
				vscodeMenus('scm/resourceGroup/context', { scmResourceGroup: group, scmViewMode: mode }),
			);
		}
	});

	it.each([1, 2])("matches VS Code's repository menu among %i repositories", (repositories) => {
		expect(scmRowMenus({ kind: 'repository', repositories }, true)).toEqual(
			vscodeMenus('scm/sourceControl', { gitOpenRepositoryCount: repositories }),
		);
	});
});
