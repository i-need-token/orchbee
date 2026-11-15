import type { ReactElement, ReactNode } from 'react';

// The tags website messages may carry: pass this object as the values of `formatMessage`.
// `<code>` marks a command or a name typed as written; `<b>` marks emphasis. The surrounding
// component styles both.
export const richText = {
	code: (chunks: readonly ReactNode[]): ReactElement => <code>{chunks}</code>,
	b: (chunks: readonly ReactNode[]): ReactElement => <b>{chunks}</b>,
};
