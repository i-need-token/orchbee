import * as React from 'react';

import type { GitPort } from '../../file-session';

/**
 * Tells the backend whether the window has focus while Source Control follows the Workspace, as
 * VS Code waits for a focused window before an automatic status refresh. Every follower reports the
 * same window, so a later unmount leaves the last report in place for the others.
 */
export function useGitClientFocus(git: GitPort | null): void {
	React.useEffect(() => {
		if (git === null) return undefined;
		const report = (): void => {
			git.focus(document.visibilityState === 'visible' && document.hasFocus());
		};
		report();
		window.addEventListener('focus', report);
		window.addEventListener('blur', report);
		document.addEventListener('visibilitychange', report);
		return () => {
			window.removeEventListener('focus', report);
			window.removeEventListener('blur', report);
			document.removeEventListener('visibilitychange', report);
		};
	}, [git]);
}
