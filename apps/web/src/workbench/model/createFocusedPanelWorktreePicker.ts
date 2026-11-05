import { workbenchMessages, type IntlShape } from '@orchbee/i18n';

import type { WorkbenchFocusedPanelWorktreeCheckoutInput } from '../actions/workbench-action-ids';
import type {
	WorkbenchSearchProviderArgumentPicker,
	WorkbenchSearchProviderArgumentRead,
} from '../types/workbench-search-provider';
import type { WorkbenchWorktrees } from '../types/workbench-worktrees';
import { resolveFocusedPanel } from '../utils/resolveFocusedPanel';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';

interface CreateFocusedPanelWorktreePickerOptions {
	readonly intl: IntlShape;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly worktrees: WorkbenchWorktrees;
}

function unavailable(label: string): WorkbenchSearchProviderArgumentRead {
	return { kind: 'unavailable', label };
}

/**
 * Search's checkout choice for Open Worktree from the focused Panel, as the TUI's `Ctrl+B O` lists
 * the checkouts Git reports where that Panel's terminal stands. Each choice names the Panel it was
 * read for, so the opening acts on the discovery the person was shown. Until Git answers, and when
 * it answers with nothing to open, one disabled row says why in the sidebar menu's words.
 */
export function createFocusedPanelWorktreePicker({
	intl,
	projectWorkspaceRuntime,
	worktrees,
}: CreateFocusedPanelWorktreePickerOptions): WorkbenchSearchProviderArgumentPicker {
	const listingUnavailable = intl.formatMessage(workbenchMessages.worktreeListingUnavailable);
	return {
		kind: 'read',
		placeholder: intl.formatMessage(workbenchMessages.searchArgumentPlaceholder),
		prompt: intl.formatMessage(workbenchMessages.searchArgumentPrompt, {
			action: intl.formatMessage(workbenchMessages.worktreeOpen),
		}),
		read: async () => {
			const focused = resolveFocusedPanel(projectWorkspaceRuntime);
			if (focused === null) return unavailable(listingUnavailable);
			const { panelId, presentationId, workspaceId } = focused;
			const result = await worktrees.discover(workspaceId, { panelId, presentationId });
			if (result.kind === 'failed') {
				return unavailable(
					result.reason === 'not-repository'
						? intl.formatMessage(workbenchMessages.worktreeNotRepository)
						: listingUnavailable,
				);
			}
			const { checkouts } = result.discovery;
			if (checkouts.length === 0) {
				return unavailable(intl.formatMessage(workbenchMessages.worktreeNoOtherCheckouts));
			}
			return {
				kind: 'options',
				options: checkouts.map((checkout, index) => {
					const input: WorkbenchFocusedPanelWorktreeCheckoutInput = {
						panelId,
						path: checkout.path,
						presentationId,
						workspaceId,
					};
					return {
						description: checkout.branch === null ? undefined : checkout.path,
						// Git's order, kept when a query sorts the matches by identity.
						id: String(index).padStart(4, '0'),
						input,
						keywords: [checkout.path],
						label: checkout.branch ?? checkout.path,
					};
				}),
			};
		},
		readFailed: listingUnavailable,
		reading: intl.formatMessage(workbenchMessages.worktreeReading),
	};
}
