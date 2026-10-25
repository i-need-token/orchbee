import type { MessageDescriptor } from 'react-intl';
import { describe, expect, it } from 'vitest';

import { enUSMessages } from '../../locales';
import { agentIntegrationMessages } from '../agent-integrations';
import { agentObservationMessages } from '../agent-observation';
import { applicationRoutingMessages } from '../application-routing';

import { desktopApplicationLifetimeMessages } from '../desktop-application-lifetime';

import { desktopSecureKeyboardEntryMessages } from '../desktop-secure-keyboard-entry';

import { desktopWindowMessages } from '../desktop-window';
import { keyboardShortcutMessages } from '../keyboard-shortcuts';
import { projectedTerminalMessages } from '../projected-terminal';

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

	['Desktop application lifetime', desktopApplicationLifetimeMessages],

	['Desktop Secure Keyboard Entry', desktopSecureKeyboardEntryMessages],

	['Desktop window', desktopWindowMessages],
	['keyboard shortcuts', keyboardShortcutMessages],
	['projected terminal', projectedTerminalMessages],

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
				expect(descriptor.defaultMessage).toBe(
					enUSMessages[messageId as keyof typeof enUSMessages],
				);
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
