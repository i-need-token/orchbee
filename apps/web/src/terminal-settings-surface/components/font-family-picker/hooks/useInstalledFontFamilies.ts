import * as React from 'react';

import type { TerminalSettingsFontCatalog } from '../../../types/terminal-settings-surface';
import {
	classifyFontFamilies,
	type InstalledFontFamily,
} from '../utils/font-family-classification';

type InstalledFontFamilies =
	/** Nothing asked yet, or the last listing was refused; opening the list asks again. */
	| { readonly kind: 'idle' }
	| { readonly kind: 'listing' }
	| { readonly kind: 'listed'; readonly families: readonly InstalledFontFamily[] };

interface InstalledFontFamiliesView {
	readonly families: readonly InstalledFontFamily[];
	/** Asks the catalog once the list opens; a press is what lets a browser ask the person. */
	readonly list: () => void;
}

/** The installed families for the font list, asked for the first time the list opens. */
export function useInstalledFontFamilies(
	catalog: TerminalSettingsFontCatalog | undefined,
): InstalledFontFamiliesView {
	const [state, setState] = React.useState<InstalledFontFamilies>({ kind: 'idle' });
	return {
		families: state.kind === 'listed' ? state.families : [],
		list: () => {
			if (catalog === undefined || state.kind !== 'idle') return;
			setState({ kind: 'listing' });
			void catalog.listFamilies().then(
				(families) => {
					setState(
						families === null
							? { kind: 'idle' }
							: { families: classifyFontFamilies(families), kind: 'listed' },
					);
				},
				() => {
					setState({ kind: 'idle' });
				},
			);
		},
	};
}
