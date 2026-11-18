import { Combobox as ComboboxPrimitive } from '@base-ui/react/combobox';

import { cn } from '#lib';
import { ChevronDownIcon, CheckIcon } from 'lucide-react';

const Combobox = ComboboxPrimitive.Root;

/*
 * Orchbee adaptation: a trigger that stands alone, with the search field inside the list, wears the
 * select trigger's filled neutral control so a searchable choice reads like any other choice.
 */
function ComboboxTrigger({
	className,
	children,
	size = 'default',
	...props
}: ComboboxPrimitive.Trigger.Props & {
	size?: 'sm' | 'default';
}) {
	return (
		<ComboboxPrimitive.Trigger
			data-slot='combobox-trigger'
			data-size={size}
			className={cn(
				"flex w-fit items-center justify-between gap-1.5 rounded-lg border border-border bg-secondary py-1 pr-2 pl-2.5 text-sm whitespace-nowrap text-secondary-foreground transition-colors outline-none select-none hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-popup-open:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_8%)] data-[size=default]:h-8 data-[size=sm]:h-7 data-[size=sm]:rounded-[min(var(--radius-md),10px)] *:data-[slot=combobox-value]:line-clamp-1 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
				className,
			)}
			{...props}
		>
			{children}
			<ChevronDownIcon className='pointer-events-none size-3.5 text-muted-foreground' />
		</ComboboxPrimitive.Trigger>
	);
}

/*
 * Orchbee adaptation: a search field that heads the list, as in the reference's font list: full
 * width and borderless, divided from the entries by one line, instead of an outlined input group.
 * Its text starts where the entries' text starts.
 */
function ComboboxSearch({ className, ...props }: ComboboxPrimitive.Input.Props) {
	return (
		<ComboboxPrimitive.Input
			data-slot='combobox-search'
			className={cn(
				'h-9 w-full min-w-0 border-b border-border bg-transparent px-2.5 text-sm outline-none placeholder:text-muted-foreground',
				className,
			)}
			{...props}
		/>
	);
}

/*
 * Orchbee adaptation: the list sits on the modal layer, because a choice opened from a dialog must
 * paint above it, and it may grow wider than its trigger to fit its entries.
 */
function ComboboxContent({
	className,
	side = 'bottom',
	sideOffset = 6,
	align = 'start',
	alignOffset = 0,
	anchor,
	...props
}: ComboboxPrimitive.Popup.Props &
	Pick<
		ComboboxPrimitive.Positioner.Props,
		'side' | 'align' | 'sideOffset' | 'alignOffset' | 'anchor'
	>) {
	return (
		<ComboboxPrimitive.Portal>
			<ComboboxPrimitive.Positioner
				side={side}
				sideOffset={sideOffset}
				align={align}
				alignOffset={alignOffset}
				anchor={anchor}
				className='isolate z-(--ob-ref-layer-modal)'
			>
				<ComboboxPrimitive.Popup
					data-slot='combobox-content'
					className={cn(
						'group/combobox-content relative max-h-(--available-height) w-max max-w-(--available-width) min-w-(--anchor-width) origin-(--transform-origin) overflow-hidden rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10 duration-100 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95',
						className,
					)}
					{...props}
				/>
			</ComboboxPrimitive.Positioner>
		</ComboboxPrimitive.Portal>
	);
}

function ComboboxList({ className, ...props }: ComboboxPrimitive.List.Props) {
	return (
		<ComboboxPrimitive.List
			data-slot='combobox-list'
			className={cn(
				'no-scrollbar max-h-[min(calc(--spacing(72)---spacing(9)),calc(var(--available-height)---spacing(9)))] scroll-py-1 overflow-y-auto overscroll-contain p-1 data-empty:p-0',
				className,
			)}
			{...props}
		/>
	);
}

function ComboboxItem({ className, children, ...props }: ComboboxPrimitive.Item.Props) {
	return (
		<ComboboxPrimitive.Item
			data-slot='combobox-item'
			className={cn(
				"relative flex w-full cursor-default items-center gap-2 rounded-md py-1 pr-8 pl-1.5 text-sm outline-hidden select-none data-highlighted:bg-accent data-highlighted:text-accent-foreground not-data-[variant=destructive]:data-highlighted:**:text-accent-foreground data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
				className,
			)}
			{...props}
		>
			{children}
			<ComboboxPrimitive.ItemIndicator
				render={
					<span className='pointer-events-none absolute right-2 flex size-4 items-center justify-center' />
				}
			>
				<CheckIcon className='pointer-events-none' />
			</ComboboxPrimitive.ItemIndicator>
		</ComboboxPrimitive.Item>
	);
}

function ComboboxGroup({ className, ...props }: ComboboxPrimitive.Group.Props) {
	return (
		<ComboboxPrimitive.Group data-slot='combobox-group' className={cn(className)} {...props} />
	);
}

/* Orchbee adaptation: a group's label starts on its entries' text, not two pixels past it. */
function ComboboxLabel({ className, ...props }: ComboboxPrimitive.GroupLabel.Props) {
	return (
		<ComboboxPrimitive.GroupLabel
			data-slot='combobox-label'
			className={cn('px-1.5 py-1.5 text-xs text-muted-foreground', className)}
			{...props}
		/>
	);
}

function ComboboxCollection({ ...props }: ComboboxPrimitive.Collection.Props) {
	return <ComboboxPrimitive.Collection data-slot='combobox-collection' {...props} />;
}

export {
	Combobox,
	ComboboxContent,
	ComboboxList,
	ComboboxItem,
	ComboboxGroup,
	ComboboxLabel,
	ComboboxCollection,
	ComboboxSearch,
	ComboboxTrigger,
};
