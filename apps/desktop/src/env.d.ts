/// <reference types="@rsbuild/core/types" />

interface ImportMetaEnv {
	readonly DEV: boolean;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
