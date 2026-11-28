import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import { scmRowActions, type ScmRowCommands } from '../actions/scmRowActions';
import type { SourceControlModel } from '../model/SourceControlModel';
import type { SourceControlNotifications } from '../types/source-control-ports';

interface UseScmRowActionsOptions {
	readonly model: SourceControlModel | null;
	readonly notifications: SourceControlNotifications;
	readonly runtime: ActionRuntime;
	/** Source Control's own scope, which the row Actions register in. */
	readonly scope: ActionScope;
	readonly session: FileWorkspaceSession | null;
}

/** Registers the rows' Actions while the view shows a Workspace, and returns what the rows run. */
export function useScmRowActions({
	model,
	notifications,
	runtime,
	scope,
	session,
}: UseScmRowActionsOptions): ScmRowCommands {
	const intl = useOrchbeeIntl();
	const actions = React.useMemo(() => scmRowActions(runtime, scope), [runtime, scope]);
	React.useEffect(() => {
		if (model === null || session === null) return undefined;
		const registration = actions.register({ intl, model, notifications, session });
		return () => registration.dispose();
	}, [actions, intl, model, notifications, session]);
	return actions.commands;
}
