/**
 * The build a window marks in its sidebar footer: development or test, with the checkout a
 * development build came from. A release window has none.
 */
export interface WorkbenchEnvironmentMarker {
	readonly environment: 'development' | 'test';
	readonly checkoutName: string | null;
}
