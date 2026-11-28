export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-source-control-${string}`]: string | number | undefined;
	}
}
