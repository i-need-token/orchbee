import type {
	ActionAvailability,
	PreparedActionInvocation,
	UnboundActionInvocation,
} from '@orchbee/actions';
import type { ProductIconName } from '@orchbee/ui/components';

export interface WorkbenchSearchArgumentOption {
	readonly description?: string | undefined;
	readonly id: string;
	readonly input: unknown;
	readonly keywords: readonly string[];
	readonly label: string;
}

export interface WorkbenchSearchArgumentPicker {
	readonly options: readonly WorkbenchSearchArgumentOption[];
	readonly placeholder: string;
	readonly prompt: string;
}

type WorkbenchSearchResultInvocation =
	| {
			readonly kind: 'ready';
			readonly prepared: PreparedActionInvocation;
	  }
	| {
			readonly kind: 'requires-input';
			readonly picker?: WorkbenchSearchArgumentPicker | undefined;
			readonly unbound: UnboundActionInvocation;
	  };

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
	readonly providerId: 'actions' | 'settings';
	readonly recommended: boolean;
	readonly shortcut?: string | undefined;
	readonly surfaceDisabledReason?: string | undefined;
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
