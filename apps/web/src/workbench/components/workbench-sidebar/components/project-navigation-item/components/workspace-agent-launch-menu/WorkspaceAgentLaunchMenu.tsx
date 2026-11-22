import {
	Combobox,
	ComboboxButton,
	ComboboxContent,
	ComboboxItem,
	ComboboxList,
	ComboboxSearch,
} from '@orchbee/ui/components';
import { useRef, useState, type ComponentProps, type ReactNode } from 'react';

import type { WorkbenchAgentStartWorkspace } from '../../../../../../types/workbench-agent-start';
import type {
	WorkspaceAgentChoice,
	WorkspaceAgentMenu,
} from '../../../../hooks/useWorkspaceAgentMenu';
import { resolveNavigationRowMenuReturn } from '../../utils/navigation-row-menu';
import styles from './WorkspaceAgentLaunchMenu.module.css';

/** The shared menu primitive's offset from its trigger, which the row's other menus keep. */
const TRIGGER_OFFSET = 4;

interface WorkspaceAgentLaunchMenuProps {
	readonly menu: WorkspaceAgentMenu;
	/** The row's + tool, which the row draws, as its other menus' triggers. */
	readonly trigger: ComponentProps<typeof ComboboxButton>['render'];
	readonly triggerContent: ReactNode;
	readonly workspace: WorkbenchAgentStartWorkspace;
}

/**
 * A Workspace row's +: it lists the installed Agents below it, the Recent first, under a search field
 * that keeps the keyboard. Enter or a click starts the Agent in a new Tab of the Workspace. Every
 * close clears the search, so the list opens whole.
 */
export function WorkspaceAgentLaunchMenu({
	menu,
	trigger: triggerElement,
	triggerContent,
	workspace,
}: WorkspaceAgentLaunchMenuProps): ReactNode {
	const trigger = useRef<HTMLButtonElement>(null);
	const [open, setOpen] = useState(false);
	const [query, setQuery] = useState('');
	const choices = menu.choices(query);
	// As the row's other menus return focus: to the + while the pointer holds the row, else to the
	// row after a keyboard close.
	const finalFocus = (closeType: string): HTMLElement | false => {
		const button = trigger.current;
		const row = button?.closest<HTMLElement>('[data-navigation-reorder-row]') ?? null;
		if (button === null || row === null || !button.isConnected) return false;
		return resolveNavigationRowMenuReturn({
			closeType,
			keyboardOpened: false,
			row,
			trigger: button,
		});
	};
	return (
		<Combobox
			autoHighlight
			filter={null}
			inputValue={query}
			isItemEqualToValue={(item: WorkspaceAgentChoice, value: WorkspaceAgentChoice) =>
				item.providerId === value.providerId
			}
			items={choices}
			itemToStringLabel={(choice: WorkspaceAgentChoice) => choice.label}
			open={open}
			value={null}
			onInputValueChange={setQuery}
			onOpenChange={(next) => {
				if (next) menu.refresh();
				else setQuery('');
				setOpen(next);
			}}
			onValueChange={(choice: WorkspaceAgentChoice | null) => {
				if (choice === null) return;
				menu.start(workspace.id, choice.providerId);
				setQuery('');
				setOpen(false);
			}}
		>
			<ComboboxButton ref={trigger} render={triggerElement}>
				{triggerContent}
			</ComboboxButton>
			<ComboboxContent
				align='start'
				anchor={trigger}
				aria-label={menu.title(workspace)}
				className={styles['popup']}
				finalFocus={finalFocus}
				side='bottom'
				sideOffset={TRIGGER_OFFSET}
			>
				<ComboboxSearch
					aria-label={menu.searchLabel}
					placeholder={menu.searchLabel}
					spellCheck={false}
				/>
				{choices.length === 0 ? <p className={styles['empty']}>{menu.emptyLabel(query)}</p> : null}
				<ComboboxList>
					{(choice: WorkspaceAgentChoice) => (
						<ComboboxItem key={choice.providerId} value={choice}>
							{menu.mark(workspace.id, choice.providerId)}
							{choice.label}
						</ComboboxItem>
					)}
				</ComboboxList>
			</ComboboxContent>
		</Combobox>
	);
}
