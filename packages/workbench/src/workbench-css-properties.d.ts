export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-workbench-${string}`]: string | number | undefined;
	}
}
