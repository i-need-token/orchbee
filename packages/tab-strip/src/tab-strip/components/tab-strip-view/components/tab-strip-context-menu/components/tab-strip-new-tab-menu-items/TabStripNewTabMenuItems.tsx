import {
	ContextMenuGroup,
	ContextMenuItem,
	ContextMenuLabel,
	ContextMenuSeparator,
	ContextMenuShortcut,
	ProductIcon,
} from '@orchbee/ui/components';
import { Fragment, type ReactNode, useState } from 'react';

import type {
	TabStripNewTabChoice,
	TabStripNewTabSection,
} from '../../../../../../types/tab-strip-options';

interface TabStripNewTabMenuItemsProps {
	readonly onChoose: (choice: TabStripNewTabChoice) => void;
	readonly sections: readonly TabStripNewTabSection[] | undefined;
}

/**
 * The New Tab menu's rows, section by section: a separator between sections and a section's heading
 * above its choices. Mounted with the menu, it keeps the choices the menu opened with, so a later
 * change of the host's choices moves no row under the pointer.
 */
export function TabStripNewTabMenuItems({
	onChoose,
	sections,
}: TabStripNewTabMenuItemsProps): ReactNode {
	const [shown] = useState(() => (sections ?? []).filter((section) => section.choices.length > 0));
	return shown.map((section, index) => (
		<Fragment key={section.id}>
			{index === 0 ? null : <ContextMenuSeparator />}
			<ContextMenuGroup>
				{section.label === undefined ? null : <ContextMenuLabel>{section.label}</ContextMenuLabel>}
				{section.choices.map((choice) => (
					<ContextMenuItem key={choice.id} onClick={() => onChoose(choice)}>
						<ProductIcon name={choice.icon} size='menu-leading' />
						{choice.label}
						{choice.note === undefined ? null : (
							<ContextMenuShortcut>{choice.note}</ContextMenuShortcut>
						)}
					</ContextMenuItem>
				))}
			</ContextMenuGroup>
		</Fragment>
	));
}
