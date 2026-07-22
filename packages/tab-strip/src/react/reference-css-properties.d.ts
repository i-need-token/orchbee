export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ct-${string}`]: string | number | undefined;
	}
}
