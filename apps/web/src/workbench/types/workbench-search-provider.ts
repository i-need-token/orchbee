import type {
	ActionAvailability,
	ActionDisposable,
	PreparedActionInvocation,
	UnboundActionInvocation,
} from '@orchbee/actions';
import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type { WorkbenchSearchFilter } from './workbench-search';

interface WorkbenchSearchProviderContext {
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

/** What a picker read: its options, or why it has none, which shows as one disabled row. */
export type WorkbenchSearchProviderArgumentRead =
	| {
			readonly kind: 'options';
			readonly options: readonly WorkbenchSearchProviderArgumentOption[];
	  }
	| { readonly kind: 'unavailable'; readonly label: string };

export type WorkbenchSearchProviderArgumentPicker = {
	readonly placeholder: string;
	readonly prompt: string;
} & (
	| {
			readonly kind: 'options';
			readonly options: readonly WorkbenchSearchProviderArgumentOption[];
	  }
	/**
	 * Options read each time the picker opens; the signal aborts once it closes or goes back. Until
	 * the read settles one disabled row says `reading`, and a read that rejects says `readFailed`.
	 */
	| {
			readonly kind: 'read';
			readonly read: (signal: AbortSignal) => Promise<WorkbenchSearchProviderArgumentRead>;
			readonly readFailed: string;
			readonly reading: string;
	  }
);

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
