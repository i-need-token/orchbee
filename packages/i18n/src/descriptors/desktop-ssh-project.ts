import { defineMessages } from 'react-intl';

import { enUSDesktopSshProjectMessages } from '../locales';

export const desktopSshProjectMessages = defineMessages({
	cancel: message('desktop.sshProject.actions.cancel'),
	close: message('desktop.sshProject.actions.close'),
	connect: message('desktop.sshProject.actions.connect'),
	connectionDescription: message('desktop.sshProject.connection.description'),
	connectionTitle: message('desktop.sshProject.connection.title'),
	continue: message('desktop.sshProject.actions.continue'),
	description: message('desktop.sshProject.description'),
	failureBusy: message('desktop.sshProject.failure.busy'),
	failureConnection: message('desktop.sshProject.failure.connection'),
	failureControl: message('desktop.sshProject.failure.control'),
	failureProjectPath: message('desktop.sshProject.failure.projectPath'),
	failureStorage: message('desktop.sshProject.failure.storage'),
	failureTarget: message('desktop.sshProject.failure.target'),
	failureTitle: message('desktop.sshProject.failure.title'),
	failureUnsupported: message('desktop.sshProject.failure.unsupported'),
	failureWorkbench: message('desktop.sshProject.failure.workbench'),
	fieldConfigurationFilePath: message('desktop.sshProject.fields.configurationFilePath'),
	fieldConfigurationFilePathDescription: message(
		'desktop.sshProject.fields.configurationFilePathDescription',
	),
	fieldConfigurationFilePathPlaceholder: message(
		'desktop.sshProject.fields.configurationFilePathPlaceholder',
	),
	fieldDestination: message('desktop.sshProject.fields.destination'),
	fieldDestinationPlaceholder: message('desktop.sshProject.fields.destinationPlaceholder'),
	fieldName: message('desktop.sshProject.fields.name'),
	fieldNamePlaceholder: message('desktop.sshProject.fields.namePlaceholder'),
	fieldProjectPath: message('desktop.sshProject.fields.projectPath'),
	fieldProjectPathDescription: message('desktop.sshProject.fields.projectPathDescription'),
	fieldProjectPathPlaceholder: message('desktop.sshProject.fields.projectPathPlaceholder'),
	progressCommittingProject: message('desktop.sshProject.progress.committingProject'),
	progressConnecting: message('desktop.sshProject.progress.connecting'),
	progressInspectingProject: message('desktop.sshProject.progress.inspectingProject'),
	progressRecordingEnvironment: message('desktop.sshProject.progress.recordingEnvironment'),
	progressResolvingTarget: message('desktop.sshProject.progress.resolvingTarget'),
	promptAuthenticationDescription: message('desktop.sshProject.prompt.authenticationDescription'),
	promptAuthenticationTitle: message('desktop.sshProject.prompt.authenticationTitle'),
	promptHostDescription: message('desktop.sshProject.prompt.hostDescription'),
	promptHostTitle: message('desktop.sshProject.prompt.hostTitle'),
	promptResponse: message('desktop.sshProject.prompt.response'),
	retry: message('desktop.sshProject.actions.retry'),
	title: message('desktop.sshProject.title'),
});

function message(id: keyof typeof enUSDesktopSshProjectMessages) {
	return { defaultMessage: enUSDesktopSshProjectMessages[id], id };
}
