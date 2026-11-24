import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import {
	webApplicationRootRoute,
	type WebApplicationRouteContext,
} from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	// No router type is registered, so the Router types this context as `any`; naming its type here
	// makes every input below type checked.
	const { application, coordinator }: WebApplicationRouteContext =
		webApplicationRootRoute.useRouteContext();
	const {
		actionRuntime,
		agentNameSource,
		agentObservationSource,
		applicationLocaleController,
		authority,
		clientBuild,
		clientUpdate,
		environmentMarker,
		keyboardShortcutSettingsController,
		layoutStorage,
		macosTerminalInputSettingsController,
		machines,
		openExternal,
		otherMachines,
		projectFolderAcquisition,
		projectWorkspaceRuntime,
		searchOffered,
		shortcutPlatform,
		terminalDefaultSettingsControl,
		terminalFeedbackSettingsController,
		terminalInteractionSettingsController,
		terminalLaunchProfileClient,
		terminalSettingsController,
		terminalSurface,
		workspaceLocationSource,
		...unforwarded
	} = application;
	// Most inputs are optional, so one this route left out would vanish without a type error: an
	// input added to the application fails type checking here until it is forwarded below.
	unforwarded satisfies Record<PropertyKey, never>;
	return (
		<WebApplication
			actionRuntime={actionRuntime}
			agentNameSource={agentNameSource}
			agentObservationSource={agentObservationSource}
			applicationLocaleController={applicationLocaleController}
			authority={authority}
			clientBuild={clientBuild}
			clientUpdate={clientUpdate}
			environmentMarker={environmentMarker}
			keyboardShortcutSettingsController={keyboardShortcutSettingsController}
			layoutStorage={layoutStorage}
			macosTerminalInputSettingsController={macosTerminalInputSettingsController}
			machines={machines}
			navigation={coordinator.navigation}
			openExternal={openExternal}
			otherMachines={otherMachines}
			projectFolderAcquisition={projectFolderAcquisition}
			projectWorkspaceRuntime={projectWorkspaceRuntime}
			searchOffered={searchOffered}
			shortcutPlatform={shortcutPlatform}
			terminalDefaultSettingsControl={terminalDefaultSettingsControl}
			terminalFeedbackSettingsController={terminalFeedbackSettingsController}
			terminalInteractionSettingsController={terminalInteractionSettingsController}
			terminalLaunchProfileClient={terminalLaunchProfileClient}
			terminalSettingsController={terminalSettingsController}
			terminalSurface={terminalSurface}
			workspaceLocationSource={workspaceLocationSource}
		/>
	);
}
