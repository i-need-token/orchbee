import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import { ExplorerGitDecorations } from '../model/ExplorerGitDecorations';

/**
 * The Explorer's Git decorations over the session it is given, which follow Source Control's
 * status while the Explorer is mounted. Another session builds others, and the ones they replace
 * stop following.
 */
export function useExplorerGitDecorations(
	session: FileWorkspaceSession | null,
): ExplorerGitDecorations | null {
	const [decorations, setDecorations] = React.useState<ExplorerGitDecorations | null>(null);
	React.useEffect(() => {
		if (session === null) return undefined;
		const built = new ExplorerGitDecorations(session);
		setDecorations(built);
		return () => {
			setDecorations(null);
			built.dispose();
		};
	}, [session]);
	return decorations;
}
