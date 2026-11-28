/** A row of a quick pick: an item to choose, or a separator with an optional label. */
export type SourceControlPickEntry<Value> =
	| {
			readonly description?: string | undefined;
			readonly id: string;
			readonly kind: 'item';
			readonly label: string;
			readonly value: Value;
	  }
	| { readonly kind: 'separator'; readonly label?: string | undefined };

type SourceControlQuickInputOutcome<Value> =
	| { readonly kind: 'accepted'; readonly value: Value }
	| { readonly kind: 'cancelled' };

/** The window's shared quick input, where Source Control asks what VS Code's Git asks there. */
export interface SourceControlQuickInput {
	pick<Value>(request: {
		readonly entries: readonly SourceControlPickEntry<Value>[];
		readonly placeholder?: string | undefined;
		/** Ends the pick as cancelled once it aborts, as when what it asks about goes away. */
		readonly signal?: AbortSignal | undefined;
		readonly title?: string | undefined;
	}): Promise<SourceControlQuickInputOutcome<Value>>;
	/** One line of text, such as a commit message VS Code asks for when the input is empty. */
	input(request: {
		/** Keeps the field open when focus leaves it; Escape still cancels. */
		readonly ignoreFocusOut?: boolean | undefined;
		readonly placeholder?: string | undefined;
		readonly prompt?: string | undefined;
		readonly title?: string | undefined;
	}): Promise<SourceControlQuickInputOutcome<string>>;
}

/** The Workbench's notification cards, where Source Control tells what VS Code's Git tells there. */
export interface SourceControlNotifications {
	notify(request: {
		readonly buttons?: readonly string[] | undefined;
		readonly message: string;
		readonly severity: 'error' | 'info' | 'warning';
	}): Promise<{ readonly index: number; readonly kind: 'chosen' } | { readonly kind: 'dismissed' }>;
}
