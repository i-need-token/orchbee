import { agentObservationMessages, type MessageDescriptor, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { StateGlyph } from '../../../../../state-glyph/StateGlyph';
import { AGENT_STATES, AGENT_STATE_WORDS } from '../../../../../state-glyph/state-words';
import { RING_CELLS, type RingAgent } from '../../agent-ring';
import styles from './AgentRing.module.css';

// The cells of the brand mark, clockwise from the top, and the hive outline in the place of the
// sixth, in the mark's own coordinates.
const CELL_PATHS = [
	'M623.43 281A18 18 0 0 1 623.43 299L575.51 382A18 18 0 0 1 559.92 391L464.08 391A18 18 0 0 1 448.49 382L400.57 299A18 18 0 0 1 400.57 281L448.49 198A18 18 0 0 1 464.08 189L559.92 189A18 18 0 0 1 575.51 198Z',
	'M815.69 614A18 18 0 0 1 815.69 632L767.77 715A18 18 0 0 1 752.18 724L656.34 724A18 18 0 0 1 640.75 715L592.83 632A18 18 0 0 1 592.83 614L640.75 531A18 18 0 0 1 656.34 522L752.18 522A18 18 0 0 1 767.77 531Z',
	'M623.43 725A18 18 0 0 1 623.43 743L575.51 826A18 18 0 0 1 559.92 835L464.08 835A18 18 0 0 1 448.49 826L400.57 743A18 18 0 0 1 400.57 725L448.49 642A18 18 0 0 1 464.08 633L559.92 633A18 18 0 0 1 575.51 642Z',
	'M431.17 614A18 18 0 0 1 431.17 632L383.25 715A18 18 0 0 1 367.66 724L271.82 724A18 18 0 0 1 256.23 715L208.31 632A18 18 0 0 1 208.31 614L256.23 531A18 18 0 0 1 271.82 522L367.66 522A18 18 0 0 1 383.25 531Z',
	'M431.17 392A18 18 0 0 1 431.17 410L383.25 493A18 18 0 0 1 367.66 502L271.82 502A18 18 0 0 1 256.23 493L208.31 410A18 18 0 0 1 208.31 392L256.23 309A18 18 0 0 1 271.82 300L367.66 300A18 18 0 0 1 383.25 309Z',
] as const;
const OUTLINE_PATH = 'M800.1 401L752.18 484H656.34L608.42 401L656.34 318H752.18Z';

// Each Agent by the name the product shows.
const PROVIDERS: Readonly<Record<RingAgent, MessageDescriptor>> = {
	claude: agentObservationMessages.providerClaudeCode,
	codex: agentObservationMessages.providerCodex,
	opencode: agentObservationMessages.providerOpenCode,
	gemini: agentObservationMessages.providerGeminiCli,
	pi: agentObservationMessages.providerPi,
};

// Each cell's place and entrance, in the order of RING_CELLS.
const PLACES = ['Top', 'LowerRight', 'Bottom', 'LowerLeft', 'UpperLeft'] as const;

/**
 * The brand mark as a ring of Agents: five cells that each show an Agent, its state and the last
 * lines of its terminal, and the hive outline where `agent start` adds the next. The page shows
 * the cells as they first appear; with script the working Agents print on, one line at a time.
 * Nothing in it takes focus.
 */
export function AgentRing(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['ring']} data-agent-ring=''>
			<svg className={styles['mark']} viewBox='200 183 624 658' aria-hidden='true'>
				{CELL_PATHS.map((path, index) => (
					<path key={path} className={styles[`shape${PLACES[index] ?? ''}`]} d={path} />
				))}
				<path className={styles['outline']} d={OUTLINE_PATH} />
			</svg>
			{RING_CELLS.map((cell, index) => (
				<div
					key={cell.id}
					className={styles[`cell${PLACES[index] ?? ''}`]}
					data-ring-cell={cell.id}
				>
					<span className={styles['head']}>
						<span className={styles['glyph']}>
							<StateGlyph state={cell.state} />
						</span>
						<span>{t(PROVIDERS[cell.id])}</span>
					</span>
					<small className={styles['state']} data-ring-state=''>
						{t(AGENT_STATE_WORDS[cell.state])}
					</small>
					<div className={styles['lines']} data-ring-lines=''>
						{cell.lines.slice(0, cell.shown).map((line) => (
							<div key={line} className={styles['line']}>
								{line}
							</div>
						))}
					</div>
				</div>
			))}
			<div className={styles['start']}>
				<b className={styles['plus']}>+</b>agent start
			</div>
			<template data-ring-line=''>
				<div className={styles['line']} />
			</template>
			<template data-ring-words=''>
				{AGENT_STATES.map((state) => (
					<span key={state} data-key={state}>
						{t(AGENT_STATE_WORDS[state])}
					</span>
				))}
			</template>
		</div>
	);
}
