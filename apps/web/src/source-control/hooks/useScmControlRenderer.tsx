import type { ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	formatKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import type { ScmGitCommands } from '../actions/scmGitActions';
import { ScmActionButtonView } from '../components/scm-action-button-view/ScmActionButtonView';
import { ScmInputEditor } from '../components/scm-input-editor/ScmInputEditor';
import { scmActionButton } from '../model/scm-action-button';
import { scmActivity } from '../model/scmActivity';
import { scmHeadShortName } from '../model/scm-repository-row';
import type { ScmTreeItem } from '../model/scm-tree';
import type { SourceControlModel, SourceControlState } from '../model/SourceControlModel';

/** The key that accepts the commit input, VS Code's `scm.acceptInput`, which its placeholder names. */
const ACCEPT_CHORD: KeyboardShortcutChord = { code: 'Enter', modifiers: ['mod'] };

/** How many changes one status reads at most, VS Code's `git.statusLimit`, past which input warns. */
const STATUS_LIMIT = 10_000;

/** A repository's commit input or action button row. */
export type ScmControlItem = Extract<ScmTreeItem, { readonly kind: 'button' | 'input' }>;

/** Which control: a repository's commit input or its action button. */
type ScmControlTarget = Pick<ScmControlItem, 'kind' | 'repository'>;

interface UseScmControlRendererOptions {
	/** The scope the commit inputs' keyboard bindings resolve their Actions in. */
	readonly actionScope: ActionScope;
	readonly gitCommands: ScmGitCommands;
	readonly model: SourceControlModel | null;
	readonly platform: KeyboardShortcutPlatform;
	readonly state: SourceControlState | null;
}

interface ScmControls {
	/** Moves focus into the control while its row is drawn; false while it is not. */
	readonly focus: (target: ScmControlTarget) => boolean;
	/** Draws a repository's commit input or action button from the state of the moment. */
	readonly render: (item: ScmControlItem) => React.ReactNode;
}

function controlKey({ kind, repository }: ScmControlTarget): string {
	return `${kind}\0${repository}`;
}

/**
 * The Changes tree's control rows, VS Code's SCM input and action button: the input commits on
 * `Mod+Enter`, names the branch in its placeholder and warns past the status limit; the button runs
 * what VS Code's Git action button offers. Each drawn control gives what focuses it, which opening
 * its row uses.
 */
export function useScmControlRenderer({
	actionScope,
	gitCommands,
	model,
	platform,
	state,
}: UseScmControlRendererOptions): ScmControls {
	const intl = useOrchbeeIntl();
	const [targets] = React.useState(() => new Map<string, () => void>());
	const focus = useMemoizedFn((control: ScmControlTarget): boolean => {
		const target = targets.get(controlKey(control));
		target?.();
		return target !== undefined;
	});
	const render = React.useMemo(() => {
		const accept = formatKeyboardShortcutChord(ACCEPT_CHORD, platform);
		return (item: ScmControlItem): React.ReactNode => {
			if (model === null || state === null) return null;
			const { repository } = item;
			const summary = state.repositories.find(
				(entry) => entry.summary.repository === repository,
			)?.summary;
			if (summary === undefined) return null;
			const key = controlKey(item);
			const onFocusTarget = (target: (() => void) | null): void => {
				if (target === null) targets.delete(key);
				else targets.set(key, target);
			};
			const activity = scmActivity(summary.busy, state.activity.get(repository));
			const input = model.input(repository);
			if (item.kind === 'input') {
				const branch = scmHeadShortName(summary);
				return (
					<ScmInputEditor
						actionScope={actionScope}
						input={input}
						key={key}
						placeholder={
							branch === null
								? intl.formatMessage(sourceControlMessages.inputPlaceholderNoBranch, {
										key: accept,
									})
								: intl.formatMessage(sourceControlMessages.inputPlaceholder, {
										branch,
										key: accept,
									})
						}
						platform={platform}
						readOnly={activity.committing}
						validation={
							summary.truncated
								? intl.formatMessage(sourceControlMessages.inputTooManyChanges, {
										count: STATUS_LIMIT,
									})
								: null
						}
						onAccept={() => gitCommands.run(repository, { kind: 'commit' }, 'keyboard')}
						onFocusTarget={onFocusTarget}
						onHeight={(height) => model.setInputHeight(repository, height)}
					/>
				);
			}
			const button = scmActionButton({
				committing: activity.committing,
				postCommit: activity.postCommit,
				remembered: input.remembered ?? null,
				remotes: state.remotes.get(repository) ?? [],
				smartCommit: {
					enabled: state.settings.enableSmartCommit,
					suggest: state.settings.suggestSmartCommit,
				},
				summary,
				syncing: activity.syncing,
			});
			if (button === null) return null;
			return (
				<ScmActionButtonView
					button={button}
					key={key}
					onFocusTarget={onFocusTarget}
					onRun={(command) => gitCommands.run(repository, command, 'button')}
				/>
			);
		};
	}, [actionScope, gitCommands, intl, model, platform, state, targets]);
	return { focus, render };
}
