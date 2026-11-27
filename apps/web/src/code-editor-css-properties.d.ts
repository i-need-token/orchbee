export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-code-editor-${string}`]: string | number | undefined;
	}
}
