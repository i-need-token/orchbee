/** A target that the terminal can ask its host to open. */
export type TerminalOpenTarget =
	| { readonly kind: 'url'; readonly url: string }
	| {
			readonly kind: 'file';
			readonly path: string;
			readonly line?: number;
			readonly column?: number;
	  }
	| { readonly kind: 'unknown'; readonly value: string };

/** Serializable contribution to an application-owned terminal context menu. */
export type ContextMenuItem =
	| {
			readonly kind: 'action';
			readonly actionId: string;
			readonly group: string;
			readonly order: number;
	  }
	| { readonly kind: 'separator'; readonly group: string; readonly order: number }
	| {
			readonly kind: 'submenu';
			readonly label: string;
			readonly group: string;
			readonly order: number;
			readonly children: readonly ContextMenuItem[];
	  };
