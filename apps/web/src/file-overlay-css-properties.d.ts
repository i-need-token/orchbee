export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-file-overlay-${string}`]: string | number | undefined;
	}
}
