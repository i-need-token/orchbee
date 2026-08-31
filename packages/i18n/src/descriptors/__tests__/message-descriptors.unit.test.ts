import type { MessageDescriptor } from 'react-intl';
import { describe, expect, it } from 'vitest';

import { enUSMessages, type OrchbeeMessageId } from '../../locales';
import { agentIntegrationMessages } from '../agent-integrations';
import { agentObservationMessages } from '../agent-observation';
import { applicationRoutingMessages } from '../application-routing';
import { browserAuthenticationMessages } from '../browser-authentication';
import { desktopApplicationLifetimeMessages } from '../desktop-application-lifetime';
import { desktopBrowserAccessMessages } from '../desktop-browser-access';
import { desktopSecureKeyboardEntryMessages } from '../desktop-secure-keyboard-entry';
import { desktopSshProjectMessages } from '../desktop-ssh-project';
import { desktopWindowMessages } from '../desktop-window';
import { keyboardShortcutMessages } from '../keyboard-shortcuts';
import { projectedTerminalMessages } from '../projected-terminal';
import { remoteDirectoryBrowserMessages } from '../remote-directory-browser';
import { sshConnectionManagementMessages } from '../ssh-connection-management';
import { terminalSettingsMessages } from '../terminal-settings';
import { workbenchMessages } from '../workbench';
import { workbenchTabMessages } from '../workbench-tabs';

const descriptorCollections: readonly (readonly [
	owner: string,
	descriptors: Readonly<Record<string, MessageDescriptor>>,
])[] = [
	['agent integrations', agentIntegrationMessages],
	['agent observation', agentObservationMessages],
	['application routing', applicationRoutingMessages],
	['Browser authentication', browserAuthenticationMessages],
	['Desktop application lifetime', desktopApplicationLifetimeMessages],
	['Desktop Browser access', desktopBrowserAccessMessages],
	['Desktop Secure Keyboard Entry', desktopSecureKeyboardEntryMessages],
	['Desktop SSH Project', desktopSshProjectMessages],
	['Desktop window', desktopWindowMessages],
	['keyboard shortcuts', keyboardShortcutMessages],
	['projected terminal', projectedTerminalMessages],
	['remote directory browser', remoteDirectoryBrowserMessages],
	['SSH connection management', sshConnectionManagementMessages],
	['terminal settings', terminalSettingsMessages],
	['Workbench', workbenchMessages],
	['Workbench tabs', workbenchTabMessages],
];

describe('Orchbee message descriptors', () => {
	it('uses canonical English defaults for every descriptor', () => {
		for (const [owner, descriptors] of descriptorCollections) {
			for (const descriptor of Object.values(descriptors)) {
				const messageId = descriptor.id;
				if (typeof messageId !== 'string') {
					throw new TypeError(`${owner} contains a descriptor without a string ID`);
				}
				if (!Object.hasOwn(enUSMessages, messageId)) {
					throw new Error(`${owner} descriptor ${messageId} is absent from the English catalog`);
				}
				expect(descriptor.defaultMessage).toBe(enUSMessages[messageId as OrchbeeMessageId]);
			}
		}
	});

	it('assigns every descriptor ID to one owner', () => {
		const duplicateAssignments: {
			readonly messageId: string;
			readonly owner: string;
			readonly previousOwner: string;
		}[] = [];
		const ownersByMessageId = new Map<string, string>();
		for (const [owner, descriptors] of descriptorCollections) {
			for (const descriptor of Object.values(descriptors)) {
				const messageId = descriptor.id;
				if (typeof messageId !== 'string') continue;
				const previousOwner = ownersByMessageId.get(messageId);
				if (previousOwner !== undefined) {
					duplicateAssignments.push({ messageId, owner, previousOwner });
					continue;
				}
				ownersByMessageId.set(messageId, owner);
			}
		}
		expect(duplicateAssignments).toEqual([]);
	});
});
