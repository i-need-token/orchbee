import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { VirtualTreeRow } from '../../virtual-tree';
import type { ScmRowCommands } from '../actions/scmRowActions';
import { scmItemMenuTarget, scmRowMenus, type ScmMenuTarget } from '../model/scm-menus';
import {
	scmChangesRowTarget,
	scmRepositoriesRowTarget,
	type ScmRowTarget,
} from '../model/scm-row-target';
import type { ScmTreeItem } from '../model/scm-tree';
import type { SourceControlModel } from '../model/SourceControlModel';
import type { SourceControlMenuGroup } from '../types/source-control-menu';
import { selectedTreeElements } from '../utils/selectedTreeElements';
import type { SourceControlCommands } from './useSourceControlActions';

interface UseScmMenuGroupsOptions {
	readonly commands: SourceControlCommands;
	readonly model: SourceControlModel | null;
	readonly rowCommands: ScmRowCommands;
}

/** The context menus of the view's two trees, each for the row a request is for. */
interface ScmMenuGroups {
	/** A Changes row's menu, or View & Sort's choices on the empty space below the rows. */
	readonly changes: (
		row: VirtualTreeRow<string, ScmTreeItem> | null,
	) => readonly SourceControlMenuGroup[];
	/** A Repositories row's menu; the empty space offers none, as in VS Code. */
	readonly repositories: (
		row: VirtualTreeRow<string, string> | null,
	) => readonly SourceControlMenuGroup[];
}

// No desktop bridge reveals a file in the system's file manager yet, so no menu offers it.
const REVEALS_LOCAL_FILES = false;

/**
 * Builds the menus when they open, as VS Code does: each row's Actions resolved for what the row
 * stands for, every selected row when it is selected, with the rows that hide left out.
 */
export function useScmMenuGroups({
	commands,
	model,
	rowCommands,
}: UseScmMenuGroupsOptions): ScmMenuGroups {
	const intl = useOrchbeeIntl();
	return React.useMemo(() => {
		const rowGroups = (
			menuTarget: ScmMenuTarget,
			target: ScmRowTarget,
		): readonly SourceControlMenuGroup[] =>
			scmRowMenus(menuTarget, REVEALS_LOCAL_FILES).context.flatMap((group) => {
				const rows = group.flatMap((command) => {
					const resolved = rowCommands.resolve(command, target, 'context-menu');
					if (resolved === null) return [];
					return [
						{
							checked: false,
							disabled: resolved.disabled,
							id: command,
							label: intl.formatMessage(resolved.title),
							onSelect: () => {
								rowCommands.run(command, target, 'context-menu');
							},
						},
					];
				});
				return rows.length === 0 ? [] : [{ choice: false, commands: rows }];
			});
		return {
			changes: (row) => {
				if (model === null) return [];
				if (row === null) {
					return commands.viewAndSort().map((group) => ({ choice: true, commands: group }));
				}
				const { repositories, view } = model.state;
				const menuTarget = scmItemMenuTarget(row.element, view.mode, repositories.length);
				const target = scmChangesRowTarget(row.element, model.changes.selection.has(row.key), () =>
					selectedTreeElements(model.changes),
				);
				return menuTarget === null || target === null ? [] : rowGroups(menuTarget, target);
			},
			repositories: (row) => {
				if (model === null || row === null) return [];
				const target = scmRepositoriesRowTarget(
					row.element,
					model.repositories.selection.has(row.key),
					() => selectedTreeElements(model.repositories),
				);
				return rowGroups(
					{ kind: 'repository', repositories: model.state.repositories.length },
					target,
				);
			},
		};
	}, [commands, intl, model, rowCommands]);
}
