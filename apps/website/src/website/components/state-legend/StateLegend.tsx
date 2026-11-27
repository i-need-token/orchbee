import { useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import { AGENT_STATE_WORDS } from '../state-glyph/state-words';
import styles from './StateLegend.module.css';

/**
 * The Agent state glyphs with the product's word for each, held still as the product's legend
 * draws them, each in its state's shape. `list` sets them apart as a list of their own, with the
 * glyphs moving as the product draws them.
 */
export function StateLegend({
	states,
	layout = 'caption',
}: {
	states: readonly AgentState[];
	layout?: 'caption' | 'list';
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<span className={layout === 'list' ? styles['list'] : styles['legend']}>
			{states.map((state) => (
				<span key={state} className={styles['entry']}>
					<StateGlyph state={state} />
					<span>{t(AGENT_STATE_WORDS[state])}</span>
				</span>
			))}
		</span>
	);
}
