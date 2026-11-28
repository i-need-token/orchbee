import { sourceControlMessages, type IntlShape } from '@orchbee/i18n';

import type { GitRepositorySummary } from '../../file-session';
import type { ScmGitCommand, ScmGitCommands } from '../actions/scmGitActions';
import { scmCheckoutItem, scmRepositoryNames, scmSyncItem } from '../model/scm-repository-row';
import type { SourceControlState } from '../model/SourceControlModel';
import type { SourceControlMenuCommand } from '../types/source-control-menu';
import type {
	SourceControlPickEntry,
	SourceControlQuickInput,
} from '../types/source-control-ports';
import { scmSyncLabelText } from './scmSyncLabelText';

/** Git's `1_header` group of VS Code's `scm/title` menu, as far as it runs here. */
const REMOTE_COMMANDS: readonly ScmGitCommand[] = [
	{ kind: 'pull' },
	{ kind: 'push' },
	{ kind: 'fetch' },
];

interface ScmRemoteMenuOptions {
	readonly gitCommands: ScmGitCommands;
	readonly intl: IntlShape;
	readonly quickInput: SourceControlQuickInput;
	readonly state: SourceControlState;
}

/**
 * VS Code's repository picker for a Git command: each repository by name, described by its branch
 * and sync labels; null when dismissed. A single repository needs no question.
 */
async function chooseRepository(
	summaries: readonly GitRepositorySummary[],
	{ intl, quickInput, state }: ScmRemoteMenuOptions,
): Promise<string | null> {
	const [first, ...others] = summaries;
	if (first === undefined) return null;
	if (others.length === 0) return first.repository;
	const names = scmRepositoryNames(
		summaries.map(({ repository }) => repository),
		state.rules?.separator ?? '/',
		state.root?.state === 'present' ? state.root.path : null,
	);
	const entries = summaries.map((summary): SourceControlPickEntry<string> => {
		const { repository } = summary;
		const sync = scmSyncItem(summary, state.remotes.get(repository) ?? [], 'none');
		const labels = [
			scmCheckoutItem(summary, 'none').label,
			scmSyncLabelText(intl, sync?.label ?? null),
		];
		return {
			description: labels.filter((label) => label !== null && label !== '').join(' ') || undefined,
			id: repository,
			kind: 'item',
			label: names.get(repository)?.name ?? repository,
			value: repository,
		};
	});
	const outcome = await quickInput.pick({
		entries,
		placeholder: intl.formatMessage(sourceControlMessages.repositoryPickerPlaceholder),
	});
	return outcome.kind === 'accepted' ? outcome.value : null;
}

/**
 * The Changes title menu's Git commands, Pull, Push and Fetch. With one repository they act on it,
 * as VS Code's title menu does; among several each asks which of those it can run in, as VS Code's
 * command does from the Command Palette, and greys while it can run in none.
 */
export function scmRemoteMenuCommands(
	options: ScmRemoteMenuOptions,
): readonly SourceControlMenuCommand[] {
	const { gitCommands, intl, state } = options;
	if (state.repositories.length === 0) return [];
	return REMOTE_COMMANDS.map((command) => {
		const ready = state.repositories
			.map(({ summary }) => summary)
			.filter(({ repository }) => gitCommands.enabled(repository, command));
		return {
			checked: false,
			disabled: ready.length === 0,
			id: command.kind,
			label: intl.formatMessage(gitCommands.title(command)),
			onSelect: () => {
				void chooseRepository(ready, options).then((repository) => {
					if (repository !== null) gitCommands.run(repository, command, 'menu');
				});
			},
		};
	});
}
