import type { ActionAvailability } from '@orchbee/actions';
import type { ProductIconName } from '@orchbee/ui/components';
import type {
	WorkbenchSearchProviderArgumentOption,
	WorkbenchSearchProviderArgumentPicker,
	WorkbenchSearchProviderResultInvocation,
} from '../../../types/workbench-search-provider';

export type WorkbenchSearchArgumentOption = WorkbenchSearchProviderArgumentOption;

export type WorkbenchSearchArgumentPicker = WorkbenchSearchProviderArgumentPicker;

type WorkbenchSearchResultInvocation = WorkbenchSearchProviderResultInvocation;

export interface WorkbenchSearchResult {
	readonly actionId: string;
	readonly availability: ActionAvailability;
	readonly canonicalLabel: string;
	readonly description?: string | undefined;
	readonly groupId: string;
	readonly groupLabel: string;
	readonly groupOrder: number;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly invocation: WorkbenchSearchResultInvocation;
	readonly keywords: readonly string[];
	readonly label: string;
	readonly providerId: string;
	readonly providerPriority?: number | undefined;
	readonly recommended: boolean;
	readonly shortcut?: string | undefined;
	readonly surfaceDisabledReason?: string | undefined;
	readonly visibility?: 'always' | 'when-no-results' | undefined;
}

export interface WorkbenchSearchSection {
	readonly id: string;
	readonly label: string;
	readonly order: number;
	readonly results: readonly WorkbenchSearchResult[];
}

export interface WorkbenchSearchCatalog {
	readonly actionResults: readonly WorkbenchSearchResult[];
	readonly recommendedGroupLabel: string;
	readonly settingsResults: readonly WorkbenchSearchResult[];
}
