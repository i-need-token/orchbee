// The contract of every activation module: an `activate<Name>.ts` file beside the component it
// activates, listed in `activations.ts` or in its page's `activations.ts`. It finds its component
// by a data attribute and does nothing on a page that lacks one.
export interface ActivationContext {
	readonly document: Document;
	/** The visitor asked for reduced motion: nothing plays by itself, scenes keep their final state. */
	readonly reducedMotion: boolean;
}

export type Activation = (context: ActivationContext) => void;
