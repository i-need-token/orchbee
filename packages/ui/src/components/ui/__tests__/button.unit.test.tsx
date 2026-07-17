import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Button, buttonVariants } from '../button';

describe('Button', () => {
	it('renders the generated shadcn slot and variant classes', () => {
		const markup = renderToStaticMarkup(
			<Button size='sm' variant='outline'>
				New session
			</Button>,
		);

		expect(markup).toContain('data-slot="button"');
		expect(markup).toContain('border-border');
		expect(markup).toContain('h-7');
	});

	it('preserves Base UI disabled state', () => {
		const markup = renderToStaticMarkup(<Button disabled>Unavailable</Button>);

		expect(markup).toContain('disabled=""');
		expect(markup).toContain('data-disabled=""');
	});

	it('keeps the official shadcn variant generator public', () => {
		expect(buttonVariants({ variant: 'destructive' })).toContain('bg-destructive/10');
	});
});
