import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuShortcut,
	DropdownMenuTrigger,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import { useRef, useState, type ReactNode } from 'react';

import type {
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
} from '../../../../../../file-overlay-contract';
import {
	useFileOverlayHostCommands,
	useFileOverlayHostShortcutLabels,
	useFileOverlayHostStore,
} from '../../../../contexts/file-overlay-host-store';
import styles from './FileOverlayTitleActions.module.css';

/**
 * The end of the tab row, as VS Code's editor title: the active tab's title actions that VS Code
 * shows as buttons, then More Actions with the rest, then Expand, or Restore while the tier is full,
 * which ends the row.
 */
export function FileOverlayTitleActions(): ReactNode {
	const intl = useOrchbeeIntl();
	const commands = useFileOverlayHostCommands();
	const shortcutLabels = useFileOverlayHostShortcutLabels();
	const full = useFileOverlayHostStore((state) => state.mode.tier === 'full');
	// Any change to the tabs can change which of the Actions are offered.
	const tabs = useFileOverlayHostStore((state) => state.tabs);
	const actions = tabs.activeId === null ? null : commands.titleActions(tabs.activeId);
	return (
		<div className={styles['root']} data-slot='file-overlay-title-actions'>
			{actions?.buttons.map((button) => (
				<TitleActionButton
					disabled={button.disabled}
					icon={button.icon}
					key={button.id}
					label={button.label}
					shortcut={button.shortcut}
					onPress={button.run}
				/>
			))}
			{actions === null || actions.more.length === 0 || tabs.activeId === null ? null : (
				<MoreActions tabId={tabs.activeId} />
			)}
			<TitleActionButton
				icon={full ? 'screen-normal' : 'screen-full'}
				label={intl.formatMessage(full ? fileOverlayMessages.restore : fileOverlayMessages.expand)}
				shortcut={shortcutLabels.toggleFull}
				slot='file-overlay-toggle-full'
				onPress={commands.toggleFull}
			/>
		</div>
	);
}

interface TitleActionButtonProps {
	readonly disabled?: boolean | undefined;
	readonly icon: ProductIconName;
	readonly label: string;
	readonly shortcut: string | undefined;
	readonly slot?: string | undefined;
	readonly onPress: () => void;
}

/** A title action's icon button, whose tooltip names it with the keys that run it. */
function TitleActionButton({
	disabled,
	icon,
	label,
	shortcut,
	slot,
	onPress,
}: TitleActionButtonProps): ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-label={label}
						className={styles['action']}
						data-slot={slot}
						disabled={disabled}
						size='icon-sm'
						variant='ghost'
						onClick={onPress}
					/>
				}
			>
				<ProductIcon name={icon} />
			</TooltipTrigger>
			<TooltipContent align='end' side='bottom'>
				{label}
				{shortcut === undefined ? null : <Kbd>{shortcut}</Kbd>}
			</TooltipContent>
		</Tooltip>
	);
}

interface MoreActionsMenu {
	readonly entries: readonly FileOverlayMenuEntry[];
	readonly open: boolean;
}

const CLOSED: MoreActionsMenu = { entries: [], open: false };

/**
 * More Actions, VS Code's `…` at the end of the editor title. It takes the active tab's items as it
 * opens and keeps them while it closes. VS Code's menus give the keys back before a chosen item runs,
 * so the item finds them on this button, where a quick input it opens returns them.
 */
function MoreActions({ tabId }: { readonly tabId: string }): ReactNode {
	const intl = useOrchbeeIntl();
	const commands = useFileOverlayHostCommands();
	const triggerReference = useRef<HTMLButtonElement>(null);
	const [menu, setMenu] = useState<MoreActionsMenu>(CLOSED);
	const label = intl.formatMessage(fileOverlayMessages.moreActions);
	const onOpenChange = useMemoizedFn((open: boolean) => {
		if (open) setMenu({ entries: commands.titleActions(tabId).more, open: true });
		else setMenu((current) => ({ ...current, open: false }));
	});
	const choose = useMemoizedFn((entry: FileOverlayMenuCommand) => {
		triggerReference.current?.focus({ preventScroll: true });
		entry.run();
	});
	const finalFocus = useMemoizedFn((): HTMLElement | false => {
		const trigger = triggerReference.current;
		if (trigger === null) return false;
		const { activeElement, body } = trigger.ownerDocument;
		// A chosen item or a press outside has placed the keys already; Base UI would take them back.
		const placed =
			activeElement !== null &&
			activeElement !== body &&
			activeElement.closest('[role="menu"]') === null;
		return placed ? false : trigger;
	});
	return (
		<Tooltip>
			<DropdownMenu open={menu.open} onOpenChange={onOpenChange}>
				<DropdownMenuTrigger
					render={
						<TooltipTrigger
							render={
								<Button
									aria-label={label}
									className={styles['action']}
									data-slot='file-overlay-more-actions'
									ref={triggerReference}
									size='icon-sm'
									variant='ghost'
								/>
							}
						/>
					}
				>
					<ProductIcon name='ellipsis' />
				</DropdownMenuTrigger>
				<DropdownMenuContent
					align='end'
					finalFocus={finalFocus}
					positionerClassName={styles['layer']}
				>
					{menu.entries.map((entry) =>
						entry.kind === 'separator' ? (
							<DropdownMenuSeparator key={entry.id} />
						) : (
							<DropdownMenuItem
								disabled={entry.disabled}
								key={entry.id}
								onClick={() => choose(entry)}
							>
								{entry.label}
								{entry.shortcut === undefined ? null : (
									<DropdownMenuShortcut>{entry.shortcut}</DropdownMenuShortcut>
								)}
							</DropdownMenuItem>
						),
					)}
				</DropdownMenuContent>
			</DropdownMenu>
			<TooltipContent align='end' side='bottom'>
				{label}
			</TooltipContent>
		</Tooltip>
	);
}
