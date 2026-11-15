import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';

import { AGENT_STATES, AGENT_STATE_WORDS } from '../state-glyph/state-words';
import {
	SESSION_AGENTS,
	SUMMARY_STATES,
	sessionStrings,
	stateStringKey,
	summaryStringKey,
	type SessionStrings,
} from './session';

const SUMMARIES = {
	attention: websiteProductMessages.summaryAttention,
	working: websiteProductMessages.summaryWorking,
	done: websiteProductMessages.summaryDone,
	idle: websiteProductMessages.summaryReady,
} as const;

/**
 * The page-language words the Session shows, as `[key, text]` entries the page carries for script
 * and as the lookups a frame is made with.
 */
export function useSessionStrings(): {
	entries: readonly (readonly [string, string])[];
	strings: SessionStrings;
} {
	const { formatMessage: t } = useOrchbeeIntl();
	const counts = SESSION_AGENTS.map((_, index) => index + 1);
	const entries = [
		...AGENT_STATES.map((state) => [stateStringKey(state), t(AGENT_STATE_WORDS[state])] as const),
		...SUMMARY_STATES.flatMap((state) =>
			counts.map(
				(count) => [summaryStringKey(state, count), t(SUMMARIES[state], { count })] as const,
			),
		),
	];
	const lookup = new Map(entries);
	return { entries, strings: sessionStrings((key) => lookup.get(key)) };
}
