import {
	projectWorkbenchAgentStartRows,
	type WorkbenchAgentStartCatalogView,
} from './projectWorkbenchAgentStartRows';

/**
 * The Agents a menu starts directly, as the TUI's menus list them: the Recent, then the Installed
 * rows of Start Agent, matched against `query` as Start Agent matches its search. A provider still
 * being checked or not found is never listed.
 */
export function projectWorkbenchAgentLaunchChoices<Provider extends string>(
	catalog: WorkbenchAgentStartCatalogView<Provider>,
	query = '',
): readonly Provider[] {
	return projectWorkbenchAgentStartRows(catalog, query, false).rows.flatMap((row) =>
		row.kind === 'provider' && (row.group === 'recent' || row.group === 'installed')
			? [row.providerId]
			: [],
	);
}
