import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type { TabModel, TabStripGroupNameRequest } from '@orchbee/tab-strip';
import * as React from 'react';

import type { NameDialogBlankRule } from '../../name-dialog/NameDialog';

interface TabGroupNameDialogView {
	readonly blank: NameDialogBlankRule;
	readonly close: () => void;
	readonly messages: {
		readonly cancel: string;
		readonly close: string;
		readonly nameTooLong: string;
	};
	readonly name: string;
	readonly setName: (name: string) => void;
	readonly submit: (name: string) => void;
	readonly submitLabel: string;
	readonly title: string;
}

interface TabGroupNameDialog {
	/** Null while nothing is being named, and once what was being named has left. */
	readonly dialog: TabGroupNameDialogView | null;
	readonly open: (request: TabStripGroupNameRequest) => void;
}

function targetPresent(
	snapshot: ReturnType<TabModel['getSnapshot']>,
	target: TabStripGroupNameRequest['target'],
): boolean {
	if (target.kind === 'group') return snapshot.groups.some((group) => group.id === target.groupId);
	return snapshot.tabs.some((tab) => target.tabIds.includes(tab.id));
}

/**
 * Names a new or existing Tab group in the TUI's group name dialog, where blank leaves the group
 * unnamed. The dialog closes with its group, or once none of the Tabs it would group remain.
 */
export function useTabGroupNameDialog(tabModel: TabModel): TabGroupNameDialog {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		tabModel.subscribe,
		tabModel.getSnapshot,
		tabModel.getSnapshot,
	);
	const [request, setRequest] = React.useState<TabStripGroupNameRequest | null>(null);
	const [name, setName] = React.useState('');
	const open = useMemoizedFn((next: TabStripGroupNameRequest) => {
		setName(next.title);
		setRequest(next);
	});
	const close = useMemoizedFn(() => setRequest(null));
	const submit = useMemoizedFn((title: string) => {
		request?.commit(title);
		setRequest(null);
	});
	if (request === null || !targetPresent(snapshot, request.target)) return { dialog: null, open };
	const creating = request.kind === 'create';
	return {
		dialog: {
			blank: { help: intl.formatMessage(workbenchTabMessages.groupNameHelp), kind: 'accept' },
			close,
			messages: {
				cancel: intl.formatMessage(workbenchMessages.resourceCancel),
				close: intl.formatMessage(workbenchMessages.resourceClose),
				nameTooLong: intl.formatMessage(workbenchMessages.nameTooLong),
			},
			name,
			setName,
			submit,
			submitLabel: intl.formatMessage(
				creating ? workbenchMessages.resourceCreate : workbenchMessages.renameSubmit,
			),
			title: intl.formatMessage(
				creating
					? workbenchTabMessages.groupNameCreateTitle
					: workbenchTabMessages.groupNameRenameTitle,
			),
		},
		open,
	};
}
