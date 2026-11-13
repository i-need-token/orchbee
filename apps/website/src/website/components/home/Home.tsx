import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { ClientStrings } from '../../readClientStrings';
import type { Release } from '../../release';
import { Agents } from '../agents/Agents';
import { Anywhere } from '../anywhere/Anywhere';
import { Hero } from '../hero/Hero';
import { Install } from '../install/Install';
import { Orchestrate } from '../orchestrate/Orchestrate';
import { Outro } from '../outro/Outro';
import { Privacy } from '../privacy/Privacy';
import { Terminal } from '../terminal/Terminal';

// The home page: the hero, six sections and the close, plus the text the page script needs.
export function Home({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const strings: ClientStrings = {
		live: {
			working: t(messages.liveWorking),
			closed: t(messages.liveClosed),
			attention: t(messages.liveAttention),
		},
		claude: [
			t(messages.activityEditing),
			t(messages.activityTesting),
			t(messages.activitySummary),
			t(messages.activityPush),
		],
		reviewer: [
			t(messages.activityDiff),
			t(messages.activityLineBreaks),
			t(messages.activityNotes),
			t(messages.activityNotes),
		],
		server: t(messages.activityServer),
		closedFor: t(messages.closedFor, { clock: '{clock}' }),
		scenes: [
			'',
			t(messages.sceneAnywhere),
			t(messages.sceneAgents),
			t(messages.sceneOrchestrate),
			t(messages.scenePrivate),
			t(messages.sceneTerminal),
			t(messages.sceneInstall),
		],
	};
	return (
		<>
			<Hero release={release} />
			<Anywhere />
			<Agents />
			<Orchestrate />
			<Privacy />
			<Terminal />
			<Install release={release} />
			<Outro release={release} />
			<div hidden data-website-strings={JSON.stringify(strings)} />
		</>
	);
}
