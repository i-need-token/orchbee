import {
	projectWorkbenchAgentStartRows,
	type WorkbenchAgentStartCatalogView,
} from './projectWorkbenchAgentStartRows';

/** As many Agents as the TUI's New Tab menu lists before More Agents…. */
const SHORTLIST_LIMIT = 4;

/**
 * The Agents a menu offers before More Agents…, as the TUI's New Tab menu lists them: the first
 * Recent, then Installed rows of Start Agent. A provider still being checked or not found is never
 * listed.
 */
export function projectWorkbenchAgentShortlist<Provider extends string>(
	catalog: WorkbenchAgentStartCatalogView<Provider>,
): readonly Provider[] {
	return projectWorkbenchAgentStartRows(catalog, '', false)
		.rows.flatMap((row) =>
			row.kind === 'provider' && (row.group === 'recent' || row.group === 'installed')
				? [row.providerId]
				: [],
		)
		.slice(0, SHORTLIST_LIMIT);
}
