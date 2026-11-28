import {
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { ScmRowCommand } from '../../model/scm-menus';
import styles from './ScmRowActions.module.css';

/** The icon of each command a row can show as a button, as VS Code's Git extension draws it. */
const COMMAND_ICONS: Partial<Record<ScmRowCommand, ProductIconName>> = {
	clean: 'discard',
	'clean-all': 'discard',
	stage: 'add',
	'stage-all': 'add',
	unstage: 'remove',
	'unstage-all': 'remove',
};

/** One button: the command it runs and its title. */
export interface ScmRowAction {
	readonly command: ScmRowCommand;
	readonly label: string;
}

interface ScmRowActionsProps {
	readonly actions: readonly ScmRowAction[];
	readonly onRun: (command: ScmRowCommand) => void;
}

/**
 * A row's inline buttons, VS Code's `inline` menu group, left to right. The row decides when they
 * show; Tab moves from the tree to the first of the focused row's. A click leaves focus where it was,
 * as VS Code's action items do, so a question the button asks gives it back there even when the
 * button's row is gone by then.
 */
export function ScmRowActions({ actions, onRun }: ScmRowActionsProps): ReactNode {
	return (
		<span className={styles['root']}>
			{actions.map(({ command, label }) => {
				const icon = COMMAND_ICONS[command];
				if (icon === undefined) return null;
				return (
					<Tooltip key={command}>
						<TooltipTrigger
							render={
								<button
									aria-label={label}
									className={styles['action']}
									tabIndex={0}
									type='button'
									onClick={() => onRun(command)}
									onMouseDown={(event) => event.preventDefault()}
								/>
							}
						>
							<ProductIcon name={icon} size='section-action' />
						</TooltipTrigger>
						<TooltipContent side='bottom'>{label}</TooltipContent>
					</Tooltip>
				);
			})}
		</span>
	);
}
