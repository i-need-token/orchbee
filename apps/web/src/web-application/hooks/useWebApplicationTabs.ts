import { createOrchbeeIntl, workbenchTabMessages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import { TabModel } from '@orchbee/tab-strip';
import type { WorkbenchTerminalCreateInput } from '@orchbee/workbench';
import * as React from 'react';

const INITIAL_TERMINAL_COUNT = 4;

interface WebApplicationTabs {
	readonly createTerminal: (
		context: ActionExecutionContext,
		input: WorkbenchTerminalCreateInput,
	) => ActionResult<void>;
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
	const createTerminal = React.useCallback(
		(context: ActionExecutionContext, input: WorkbenchTerminalCreateInput): ActionResult<void> => {
			if (context.signal.aborted) {
				return { kind: 'cancelled' };
			}
			const number = nextTerminalSequence.current;
			context.markCommitted();
			nextTerminalSequence.current += 1;
			tabModel.addTab(
				{
					title: intl.formatMessage(workbenchTabMessages.terminalTitle, { number }),
					pinned: false,
					group: input.groupId,
				},
				{
					id: `terminal-${number}`,
					...(input.index === undefined ? {} : { index: input.index }),
				},
			);
			return { kind: 'completed' };
		},
		[intl, tabModel],
	);

	return { createTerminal, tabModel } as const;
}
