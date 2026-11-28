import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import { scmGitActions, type ScmGitCommands } from '../actions/scmGitActions';
import type { SourceControlModel } from '../model/SourceControlModel';
import type {
	SourceControlNotifications,
	SourceControlQuickInput,
} from '../types/source-control-ports';

interface UseScmGitActionsOptions {
	readonly model: SourceControlModel | null;
	readonly notifications: SourceControlNotifications;
	readonly quickInput: SourceControlQuickInput;
	readonly runtime: ActionRuntime;
	/** Source Control's own scope, which the Git Actions register in. */
	readonly scope: ActionScope;
	readonly session: FileWorkspaceSession | null;
}

/**
 * Registers the Actions that commit and reach remotes while the view shows a Workspace, and returns
 * what the commit input, the action button, the sync item and the menus run.
 */
export function useScmGitActions({
	model,
	notifications,
	quickInput,
	runtime,
	scope,
	session,
}: UseScmGitActionsOptions): ScmGitCommands {
	const intl = useOrchbeeIntl();
	const actions = React.useMemo(() => scmGitActions(runtime, scope), [runtime, scope]);
	React.useEffect(() => {
		if (model === null || session === null) return undefined;
		const registration = actions.register({ intl, model, notifications, quickInput, session });
		return () => registration.dispose();
	}, [actions, intl, model, notifications, quickInput, session]);
	return actions.commands;
}
