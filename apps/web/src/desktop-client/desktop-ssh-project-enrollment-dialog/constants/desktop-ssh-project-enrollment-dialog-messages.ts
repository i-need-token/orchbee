import { desktopSshProjectMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { DesktopSshProjectEnrollmentFailureCategory } from '../model/DesktopSshProjectEnrollmentController';

export const DESKTOP_SSH_PROJECT_ENROLLMENT_FAILURE_MESSAGES = {
	busy: desktopSshProjectMessages.failureBusy,
	connection: desktopSshProjectMessages.failureConnection,
	control: desktopSshProjectMessages.failureControl,
	'project-path': desktopSshProjectMessages.failureProjectPath,
	storage: desktopSshProjectMessages.failureStorage,
	target: desktopSshProjectMessages.failureTarget,
	unsupported: desktopSshProjectMessages.failureUnsupported,
	workbench: desktopSshProjectMessages.failureWorkbench,
} as const satisfies Readonly<
	Record<DesktopSshProjectEnrollmentFailureCategory, MessageDescriptor>
>;
