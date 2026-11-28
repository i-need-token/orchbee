import { sourceControlMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import * as React from 'react';

import type { VirtualTreeRow } from '../../virtual-tree';
import type { ScmRowCommands } from '../actions/scmRowActions';
import { ScmChangeRow } from '../components/scm-change-row/ScmChangeRow';
import { ScmRowActions, type ScmRowAction } from '../components/scm-row-actions/ScmRowActions';
import { scmItemMenuTarget, scmRowMenus, type ScmRowCommand } from '../model/scm-menus';
import type { ScmGroupId } from '../model/scm-resource';
import { scmChangesRowTarget } from '../model/scm-row-target';
import type { ScmTreeItem } from '../model/scm-tree';
import type { SourceControlModel, SourceControlState } from '../model/SourceControlModel';
import { selectedTreeElements } from '../utils/selectedTreeElements';
import type { ScmControlItem } from './useScmControlRenderer';

const GROUP_MESSAGES = {
	index: sourceControlMessages.groupIndex,
	merge: sourceControlMessages.groupMerge,
	workingTree: sourceControlMessages.groupWorkingTree,
} as const satisfies Record<ScmGroupId, MessageDescriptor>;

/** The folder holding a file, relative to its repository; empty for a file at the root. */
function relativeFolder(path: string, repository: string, separator: string): string {
	const prefix = repository.endsWith(separator) ? repository : `${repository}${separator}`;
	const relative = path.startsWith(prefix) ? path.slice(prefix.length) : path;
	const end = relative.lastIndexOf(separator);
	return end < 0 ? '' : relative.slice(0, end);
}

interface UseScmChangeRowRendererOptions {
	readonly model: SourceControlModel | null;
	/** Draws a repository's commit input or action button. */
	readonly renderControl: (item: ScmControlItem) => React.ReactNode;
	/** Draws a repository's row, which the Changes tree shows above its groups among several. */
	readonly renderRepository: (repository: string) => React.ReactNode;
	readonly rowCommands: ScmRowCommands;
	readonly state: SourceControlState | null;
}

type ScmChangeRowRenderer = (
	row: VirtualTreeRow<string, ScmTreeItem>,
	state: { readonly focused: boolean },
) => React.ReactNode;

/**
 * Draws a Changes row from plain values: a group, a folder or a file, with the inline buttons VS
 * Code's Git gives it, a repository's commit input and action button, or its row among several. A
 * button acts on the row, or on every selected row when the row is selected.
 */
export function useScmChangeRowRenderer({
	model,
	renderControl,
	renderRepository,
	rowCommands,
	state,
}: UseScmChangeRowRendererOptions): ScmChangeRowRenderer {
	const intl = useOrchbeeIntl();
	return React.useMemo(() => {
		const mode = state?.view.mode ?? 'list';
		const separator = state?.rules?.separator ?? '/';
		const repositories = state?.repositories.length ?? 0;
		return (row, { focused }) => {
			const item = row.element;
			if (item.kind === 'repository') return renderRepository(item.repository);
			if (item.kind === 'input' || item.kind === 'button') return renderControl(item);
			const menuTarget = scmItemMenuTarget(item, mode, repositories);
			if (model === null || menuTarget === null) return null;
			const actions = scmRowMenus(menuTarget, false).inline.flatMap((command): ScmRowAction[] => {
				const title = rowCommands.title(command);
				return title === null ? [] : [{ command, label: intl.formatMessage(title) }];
			});
			const { key } = row;
			const run = (command: ScmRowCommand): void => {
				const target = scmChangesRowTarget(item, model.changes.selection.has(key), () =>
					selectedTreeElements(model.changes),
				);
				if (target !== null) rowCommands.run(command, target, 'button');
			};
			const buttons = actions.length === 0 ? null : <ScmRowActions actions={actions} onRun={run} />;
			switch (item.kind) {
				case 'group':
					return (
						<ScmChangeRow
							actions={buttons}
							count={intl.formatNumber(item.resources.length)}
							description=''
							focused={focused}
							label={intl.formatMessage(GROUP_MESSAGES[item.group])}
							status={null}
							statusText=''
							title=''
						/>
					);
				case 'folder':
					return (
						<ScmChangeRow
							actions={buttons}
							count={null}
							description=''
							focused={focused}
							label={item.names.join(separator)}
							status={null}
							statusText=''
							title={item.path}
						/>
					);
				case 'resource': {
					const { path, status } = item.resource;
					return (
						<ScmChangeRow
							actions={buttons}
							count={null}
							description={mode === 'list' ? relativeFolder(path, item.repository, separator) : ''}
							focused={focused}
							label={item.name}
							status={status}
							statusText={model.statusText(status)}
							title={path}
						/>
					);
				}
			}
		};
	}, [intl, model, renderControl, renderRepository, rowCommands, state]);
}
