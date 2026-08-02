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
