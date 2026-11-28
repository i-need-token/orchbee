import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuGroup,
	DropdownMenuItem,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuSeparator,
	DropdownMenuSub,
	DropdownMenuSubContent,
	DropdownMenuSubTrigger,
	DropdownMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { SourceControlMenuCommand } from '../../types/source-control-menu';
import styles from './ScmChangesMenu.module.css';

interface ScmChangesMenuProps {
	/** The Git commands below the submenu, resolved when the menu opens. */
	readonly git: () => readonly SourceControlMenuCommand[];
	/** The View & Sort submenu's groups, resolved when the menu opens. */
	readonly viewAndSort: () => readonly (readonly SourceControlMenuCommand[])[];
}

/**
 * The Changes title's More Actions menu, VS Code's `...`: its View & Sort submenu, View as List or
 * Tree, then the list's sorts, each with the choice in force checked; then Git's Pull, Push and Fetch.
 */
export function ScmChangesMenu({ git, viewAndSort }: ScmChangesMenuProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [open, setOpen] = React.useState(false);
	const label = intl.formatMessage(sourceControlMessages.moreActions);
	return (
		<DropdownMenu open={open} onOpenChange={setOpen}>
			<Tooltip>
				<TooltipTrigger
					render={
						<DropdownMenuTrigger
							render={<button aria-label={label} className={styles['trigger']} type='button' />}
						/>
					}
				>
					<ProductIcon name='ellipsis' size='section-action' />
				</TooltipTrigger>
				<TooltipContent side='bottom'>{label}</TooltipContent>
			</Tooltip>
			{open ? (
				<DropdownMenuContent align='end'>
					<DropdownMenuSub>
						<DropdownMenuSubTrigger>
							{intl.formatMessage(sourceControlMessages.viewAndSort)}
						</DropdownMenuSubTrigger>
						<DropdownMenuSubContent>
							{viewAndSort().map((group, index) => (
								<React.Fragment key={group[0]?.id ?? index}>
									{index > 0 ? <DropdownMenuSeparator /> : null}
									<DropdownMenuRadioGroup
										value={group.find((command) => command.checked)?.id ?? null}
									>
										{group.map((command) => (
											<DropdownMenuRadioItem
												key={command.id}
												closeOnClick
												disabled={command.disabled}
												value={command.id}
												onClick={command.onSelect}
											>
												{command.label}
											</DropdownMenuRadioItem>
										))}
									</DropdownMenuRadioGroup>
								</React.Fragment>
							))}
						</DropdownMenuSubContent>
					</DropdownMenuSub>
					<ScmChangesMenuGit commands={git()} />
				</DropdownMenuContent>
			) : null}
		</DropdownMenu>
	);
}

/** The menu's Git commands with the separator above them; nothing without commands. */
function ScmChangesMenuGit({
	commands,
}: {
	readonly commands: readonly SourceControlMenuCommand[];
}): React.ReactNode {
	if (commands.length === 0) return null;
	return (
		<>
			<DropdownMenuSeparator />
			<DropdownMenuGroup>
				{commands.map((command) => (
					<DropdownMenuItem key={command.id} disabled={command.disabled} onClick={command.onSelect}>
						{command.label}
					</DropdownMenuItem>
				))}
			</DropdownMenuGroup>
		</>
	);
}
