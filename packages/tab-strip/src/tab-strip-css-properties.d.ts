export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-tab-strip-${string}`]: string | number | undefined;
	}
}
