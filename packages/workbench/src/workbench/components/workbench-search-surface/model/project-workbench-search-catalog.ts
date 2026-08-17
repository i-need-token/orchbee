import type {
	ActionDiscoveryResult,
	ActionMessageDescriptor,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { workbenchMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import { WORKBENCH_ACTION_IDS } from '../../../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../../../types/workbench-shortcut-platform';
import {
	formatWorkbenchActionShortcut,
	type WorkbenchShortcutBindings,
} from '../../../utils/workbench-shortcuts';
import type {
	WorkbenchSearchArgumentPicker,
	WorkbenchSearchCatalog,
	WorkbenchSearchResult,
} from './workbench-search-result';

interface ProjectWorkbenchSearchCatalogOptions {
	readonly intl: IntlShape;
	readonly platform: WorkbenchShortcutPlatform;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly shortcutBindings: WorkbenchShortcutBindings;
}

export function projectWorkbenchSearchCatalog({
	intl,
	platform,
	runtime,
	scope,
	shortcutBindings,
}: ProjectWorkbenchSearchCatalogOptions): WorkbenchSearchCatalog {
	const discovery = runtime.discover(
		{ scope, source: 'command-surface' },
		{
			includeDescendants: true,
			inputByActionId: {
				[WORKBENCH_ACTION_IDS.openSearch]: { filter: 'all' },
			},
		},
	);
	const needsContext = intl.formatMessage(workbenchMessages.searchActionsNeedContext);
	const actionResults = discovery.map((result) =>
		projectActionResult(result, intl, platform, shortcutBindings, needsContext),
	);
	const settingsResults = actionResults
		.filter((result) => result.actionId === WORKBENCH_ACTION_IDS.openSettings)
		.map((result) => ({
			...result,
			groupId: 'settings',
			groupLabel: intl.formatMessage(workbenchMessages.searchGroupSettings),
			groupOrder: 70,
			icon: 'settings' as const,
			id: `settings:${result.id}`,
			providerId: 'settings' as const,
			recommended: true,
		}));

	return {
		actionResults,
		recommendedGroupLabel: intl.formatMessage(workbenchMessages.searchGroupRecommendedActions),
		settingsResults,
	};
}

function projectActionResult(
	result: ActionDiscoveryResult,
	intl: IntlShape,
	platform: WorkbenchShortcutPlatform,
	shortcutBindings: WorkbenchShortcutBindings,
	needsContext: string,
): WorkbenchSearchResult {
	const picker =
		result.kind === 'requires-input' &&
		result.descriptor.id === WORKBENCH_ACTION_IDS.setTabStripOrientation
			? createTabOrientationPicker(intl)
			: undefined;
	const invocation =
		result.kind === 'ready'
			? {
					kind: 'ready' as const,
					prepared: result.invocation,
				}
			: {
					kind: 'requires-input' as const,
					...(picker === undefined ? {} : { picker }),
					unbound: result.invocation,
				};
	const categoryId = result.descriptor.category.id;

	return {
		actionId: result.descriptor.id,
		availability: result.availability,
		canonicalLabel:
			result.descriptor.title.defaultMessage ??
			formatWorkbenchActionMessage(intl, result.descriptor.title),
		...(result.descriptor.description === undefined
			? {}
			: { description: formatWorkbenchActionMessage(intl, result.descriptor.description) }),
		groupId: categoryId,
		groupLabel: formatWorkbenchActionMessage(intl, result.descriptor.category.title),
		groupOrder: actionCategoryOrder(categoryId),
		icon: actionCategoryIcon(categoryId),
		id: `action:${result.scope.id}:${result.descriptor.id}`,
		invocation,
		keywords: (result.descriptor.keywords ?? []).map((keyword) =>
			formatWorkbenchActionMessage(intl, keyword),
		),
		label: formatWorkbenchActionMessage(intl, result.descriptor.title),
		providerId: 'actions',
		recommended: isRecommendedAction(result.descriptor.id),
		...(formatWorkbenchActionShortcut(result.descriptor.id, platform, shortcutBindings) === null
			? {}
			: {
					shortcut:
						formatWorkbenchActionShortcut(result.descriptor.id, platform, shortcutBindings) ??
						undefined,
				}),
		...(result.kind === 'requires-input' && picker === undefined
			? { surfaceDisabledReason: needsContext }
			: {}),
	};
}

export function formatWorkbenchActionMessage(
	intl: IntlShape,
	message: ActionMessageDescriptor,
): string {
	const descriptor: MessageDescriptor = {
		...(message.defaultMessage === undefined ? {} : { defaultMessage: message.defaultMessage }),
		...(message.description === undefined ? {} : { description: message.description }),
		...(message.id === undefined ? {} : { id: message.id }),
	};
	return intl.formatMessage(descriptor, message.values);
}

function actionCategoryOrder(categoryId: string): number {
	switch (categoryId) {
		case 'navigation':
			return 20;
		case 'terminal':
			return 30;
		case 'tabs':
			return 40;
		case 'panels':
			return 50;
		case 'view':
			return 60;
		default:
			return 100;
	}
}

function actionCategoryIcon(categoryId: string): ProductIconName {
	switch (categoryId) {
		case 'navigation':
			return 'search';
		case 'terminal':
			return 'terminal';
		case 'tabs':
			return 'tabs';
		case 'panels':
			return 'panels';
		case 'view':
			return 'layout';
		default:
			return 'action';
	}
}

function isRecommendedAction(actionId: string): boolean {
	return (
		actionId === WORKBENCH_ACTION_IDS.createTerminal ||
		actionId === WORKBENCH_ACTION_IDS.setTabStripOrientation ||
		actionId === WORKBENCH_ACTION_IDS.toggleSidebar
	);
}

function createTabOrientationPicker(intl: IntlShape): WorkbenchSearchArgumentPicker {
	return {
		options: [
			{
				id: 'horizontal',
				input: { orientation: 'horizontal' },
				keywords: ['top'],
				label: intl.formatMessage(workbenchMessages.searchArgumentHorizontalTabs),
			},
			{
				id: 'vertical',
				input: { orientation: 'vertical' },
				keywords: ['left'],
				label: intl.formatMessage(workbenchMessages.searchArgumentVerticalTabs),
			},
		],
		placeholder: intl.formatMessage(workbenchMessages.searchArgumentPlaceholder),
		prompt: intl.formatMessage(workbenchMessages.searchArgumentPrompt, {
			action: intl.formatMessage(workbenchMessages.tabStripOrientation),
		}),
	};
}
