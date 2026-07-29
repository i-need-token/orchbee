import type {
	ActionAvailability,
	ActionDisposable,
	PreparedActionInvocation,
	UnboundActionInvocation,
} from '@orchbee/actions';
import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type { WorkbenchSearchFilter } from './workbench-search';

export interface WorkbenchSearchProviderContext {
	readonly filter: WorkbenchSearchFilter;
	readonly locale: OrchbeeLocale;
}

export interface WorkbenchSearchProviderArgumentOption {
	readonly description?: string | undefined;
	readonly id: string;
	readonly input: unknown;
	readonly keywords: readonly string[];
	readonly label: string;
}

export interface WorkbenchSearchProviderArgumentPicker {
	readonly options: readonly WorkbenchSearchProviderArgumentOption[];
	readonly placeholder: string;
	readonly prompt: string;
}

export type WorkbenchSearchProviderResultInvocation =
	| {
			readonly kind: 'ready';
			readonly prepared: PreparedActionInvocation;
	  }
	| {
			readonly kind: 'requires-input';
			readonly picker?: WorkbenchSearchProviderArgumentPicker | undefined;
			readonly unbound: UnboundActionInvocation;
	  };

export interface WorkbenchSearchProviderResult {
	readonly availability: ActionAvailability;
	readonly canonicalLabel: string;
	readonly description?: string | undefined;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly invocation: WorkbenchSearchProviderResultInvocation;
	readonly keywords: readonly string[];
	readonly label: string;
	readonly recommended?: boolean | undefined;
	readonly shortcut?: string | undefined;
	readonly surfaceDisabledReason?: string | undefined;
	readonly visibility?: 'always' | 'when-no-results' | undefined;
}

export interface WorkbenchSearchProviderSection {
	readonly id: string;
	readonly label: string;
	readonly order: number;
	readonly results: readonly WorkbenchSearchProviderResult[];
}

type WorkbenchSearchProviderOutput =
	| readonly WorkbenchSearchProviderSection[]
	| Promise<readonly WorkbenchSearchProviderSection[]>;

export interface WorkbenchSearchProvider {
	readonly filters: readonly WorkbenchSearchFilter[];
	readonly id: string;
	readonly label: string;
	readonly priority: number;
	readonly resultLimit: number;
	getDefaultSections(
		context: WorkbenchSearchProviderContext,
		signal: AbortSignal,
	): WorkbenchSearchProviderOutput;
	search(
		query: string,
		context: WorkbenchSearchProviderContext,
		signal: AbortSignal,
	): WorkbenchSearchProviderOutput;
	subscribe?(listener: () => void): ActionDisposable;
}
