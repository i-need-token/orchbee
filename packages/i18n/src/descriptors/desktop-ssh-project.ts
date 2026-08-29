import { defineMessages } from 'react-intl';

import { enUSDesktopSshProjectMessages } from '../locales';

export const desktopSshProjectMessages = defineMessages({
	attachmentDescription: message('desktop.sshProject.attachment.description'),
	attachmentDirectoryDescription: message('desktop.sshProject.attachment.directoryDescription'),
	attachmentDirectoryTitle: message('desktop.sshProject.attachment.directoryTitle'),
	attachmentTitle: message('desktop.sshProject.attachment.title'),
	cancel: message('desktop.sshProject.actions.cancel'),
	close: message('desktop.sshProject.actions.close'),
	connect: message('desktop.sshProject.actions.connect'),
	connectionDescription: message('desktop.sshProject.connection.description'),
	connectionTitle: message('desktop.sshProject.connection.title'),
	continue: message('desktop.sshProject.actions.continue'),
	description: message('desktop.sshProject.description'),
	directoryDescription: message('desktop.sshProject.directory.description'),
	directoryTitle: message('desktop.sshProject.directory.title'),
	environmentConnectionDescription: message('desktop.sshProject.environmentConnection.description'),
	environmentConnectionFailureTitle: message(
		'desktop.sshProject.environmentConnection.failureTitle',
	),
	environmentConnectionTitle: message('desktop.sshProject.environmentConnection.title'),
	failureBusy: message('desktop.sshProject.failure.busy'),
	failureConnection: message('desktop.sshProject.failure.connection'),
	failureControl: message('desktop.sshProject.failure.control'),
	failureProjectAttachment: message('desktop.sshProject.failure.projectAttachment'),
	failureProjectPath: message('desktop.sshProject.failure.projectPath'),
	failureStorage: message('desktop.sshProject.failure.storage'),
	failureTarget: message('desktop.sshProject.failure.target'),
	failureTitle: message('desktop.sshProject.failure.title'),
	failureUnsupported: message('desktop.sshProject.failure.unsupported'),
	failureWorkbench: message('desktop.sshProject.failure.workbench'),
	fieldDestination: message('desktop.sshProject.fields.destination'),
	fieldDestinationDescription: message('desktop.sshProject.fields.destinationDescription'),
	fieldDestinationPlaceholder: message('desktop.sshProject.fields.destinationPlaceholder'),
	progressCommittingProject: message('desktop.sshProject.progress.committingProject'),
	progressCommittingProjectFolder: message('desktop.sshProject.progress.committingProjectFolder'),
	progressConnecting: message('desktop.sshProject.progress.connecting'),
	progressInspectingProject: message('desktop.sshProject.progress.inspectingProject'),
	progressRecordingEnvironment: message('desktop.sshProject.progress.recordingEnvironment'),
	progressResolvingTarget: message('desktop.sshProject.progress.resolvingTarget'),
	progressSelectingDirectory: message('desktop.sshProject.progress.selectingDirectory'),
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
