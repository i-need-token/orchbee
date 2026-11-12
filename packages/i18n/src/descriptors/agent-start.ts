import { defineMessages } from 'react-intl';

import { enUSAgentStartMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSAgentStartMessages);

/** The Start Agent dialog, in the words of the TUI's Start Agent picker. */
export const agentStartMessages = defineMessages({
	cancel: message('agentStart.cancel', 'Button that closes Start Agent without starting anything.'),
	detailCantCheck: message(
		'agentStart.detail.cantCheck',
		'Secondary text of an Agent row whose command is too generic, or whose lookup failed, to tell whether it is installed.',
	),
	detailNotFound: message(
		'agentStart.detail.notFound',
		'Secondary text of an Agent row whose command the Session’s machine did not find.',
	),
	foldBoth: message(
		'agentStart.fold.both',
		'Row that opens the folded list when it holds both Agents not found and Agents that could not be checked. {count} is how many it holds.',
	),
	foldCantCheck: message(
		'agentStart.fold.cantCheck',
		'Row that opens the folded list when every Agent in it could not be checked. {count} is how many it holds.',
	),
	foldNotFound: message(
		'agentStart.fold.notFound',
		'Row that opens the folded list when every Agent in it was not found. {count} is how many it holds.',
	),
	groupInstalled: message(
		'agentStart.group.installed',
		'Heading of the Agents the Session’s machine found, apart from the Recent ones.',
	),
	groupNotChecked: message(
		'agentStart.group.notChecked',
		'Heading of the Agents listed without a lookup, because the lookup failed or is not supported.',
	),
	groupNotFound: message(
		'agentStart.group.notFound',
		'Heading of the Agents the Session’s machine did not find or cannot check.',
	),
	groupRecent: message('agentStart.group.recent', 'Heading of the Agents started most recently.'),
	list: message('agentStart.list', 'Accessible name of the list of Agents to start.'),
	opensIn: message(
		'agentStart.opensIn',
		'Label of the control that chooses where the started Agent opens.',
	),
	placeNewTab: message('agentStart.place.newTab', 'Place choice: the Agent opens in a new Tab.'),
	placeNewWorktree: message(
		'agentStart.place.newWorktree',
		'Place choice: Start first opens the New Worktree form, and the Agent starts in the Workspace it creates.',
	),
	placeSplitDown: message(
		'agentStart.place.splitDown',
		'Place choice: the focused Panel is split and the Agent opens below it.',
	),
	placeSplitRight: message(
		'agentStart.place.splitRight',
		'Place choice: the focused Panel is split and the Agent opens to its right.',
	),
	search: message(
		'agentStart.search',
		'Accessible name and placeholder of the Agent search field.',
	),
	start: message(
		'agentStart.start',
		'Button that starts the highlighted Agent in the chosen place.',
	),
	statusCouldntCheck: message(
		'agentStart.status.couldntCheck',
		'Status line when the lookup for installed Agents failed or is not supported; every Agent can still be started.',
	),
	statusLooking: message(
		'agentStart.status.looking',
		'Status line while the Session’s first lookup for installed Agents runs.',
	),
	statusNoMatch: message(
		'agentStart.status.noMatch',
		'Status line when no Agent matches the search.',
	),
	statusUnsupported: message(
		'agentStart.status.unsupported',
		'Status line when the Session’s machine cannot start Agents at all.',
	),
	title: message('agentStart.title', 'Title of the Start Agent dialog and name of its action.'),
});
