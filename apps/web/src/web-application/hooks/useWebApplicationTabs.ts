import { createOrchbeeIntl, workbenchTabMessages, type OrchbeeLocale } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

const INITIAL_TERMINAL_COUNT = 4;

interface WebApplicationTabs {
	readonly createTerminal: () => void;
	readonly tabModel: TabModel;
}

export function useWebApplicationTabs(locale: OrchbeeLocale): WebApplicationTabs {
	const intl = React.useMemo(() => createOrchbeeIntl(locale), [locale]);
	const nextTerminalSequence = React.useRef(INITIAL_TERMINAL_COUNT + 1);
	const [tabModel] = React.useState(
		() =>
			new TabModel({
				tabs: Array.from({ length: INITIAL_TERMINAL_COUNT }, (_, index) => {
					const number = index + 1;
					return {
						id: `terminal-${number}`,
						title: intl.formatMessage(workbenchTabMessages.terminalTitle, { number }),
						pinned: false,
						group: null,
					};
				}),
				activeTabId: 'terminal-1',
			}),
	);
	const createTerminal = React.useCallback(() => {
		const number = nextTerminalSequence.current;
		nextTerminalSequence.current += 1;
		tabModel.addTab(
			{
				title: intl.formatMessage(workbenchTabMessages.terminalTitle, { number }),
				pinned: false,
				group: null,
			},
			{ id: `terminal-${number}` },
		);
	}, [intl, tabModel]);

	return { createTerminal, tabModel } as const;
}
