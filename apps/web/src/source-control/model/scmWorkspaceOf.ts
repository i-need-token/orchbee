import type { GitPort } from '../../file-session';
import { ScmWorkspace } from './ScmWorkspace';

const workspaces = new WeakMap<GitPort, ScmWorkspace>();

/**
 * The session's one Source Control state, which the Source Control view and the Explorer's
 * decorations share, so each status is read once however many of them follow it. It lives as long
 * as the session's Git port.
 */
export function scmWorkspaceOf(git: GitPort): ScmWorkspace {
	let workspace = workspaces.get(git);
	if (workspace === undefined) {
		workspace = new ScmWorkspace(git);
		workspaces.set(git, workspace);
	}
	return workspace;
}
