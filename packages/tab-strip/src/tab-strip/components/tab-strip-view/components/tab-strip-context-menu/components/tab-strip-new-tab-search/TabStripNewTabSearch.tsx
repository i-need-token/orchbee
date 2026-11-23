import { MenuSearch } from '@orchbee/ui/components';
import { type ReactNode, useState } from 'react';

import { useTabStripMessages } from '../../../../../../hooks/useTabStripMessages';
import type {
	TabStripNewTabChoice,
	TabStripNewTabSection,
} from '../../../../../../types/tab-strip-options';
import { TabStripNewTabMenuItems } from '../tab-strip-new-tab-menu-items/TabStripNewTabMenuItems';

interface TabStripNewTabSearchProps {
	readonly onChoose: (choice: TabStripNewTabChoice) => void;
	readonly sections: readonly TabStripNewTabSection[] | undefined;
}

/** The New Tab menu's choices under a search field, matched by name among those it opened with. */
export function TabStripNewTabSearch({ onChoose, sections }: TabStripNewTabSearchProps): ReactNode {
	const messages = useTabStripMessages();
	const [opened] = useState(sections);
	const [query, setQuery] = useState('');
	const needle = query.trim().toLocaleLowerCase();
	const matches = (opened ?? []).map((section) => ({
		...section,
		choices: section.choices.filter((choice) => choice.label.toLocaleLowerCase().includes(needle)),
	}));
	const found = matches.some((section) => section.choices.length > 0);
	return (
		<MenuSearch
			emptyLabel={found ? undefined : messages.newTabMenuNoMatch}
			label={messages.newTabMenuSearch}
			query={query}
			onQueryChange={setQuery}
		>
			<TabStripNewTabMenuItems key={needle} sections={matches} onChoose={onChoose} />
		</MenuSearch>
	);
}
