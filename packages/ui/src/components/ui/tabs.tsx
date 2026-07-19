import { Tabs as TabsPrimitive } from '@base-ui/react/tabs';
import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentProps } from 'react';

import { cn } from '#lib';

type TabsVariant = 'default' | 'workspace';

function Tabs({
	className,
	orientation = 'horizontal',
	variant = 'default',
	...props
}: TabsPrimitive.Root.Props & { readonly variant?: TabsVariant }) {
	return (
		<TabsPrimitive.Root
			data-slot='tabs'
			data-orientation={orientation}
			data-variant={variant}
			className={cn(
				'group/tabs flex gap-2 data-horizontal:flex-col',
				variant === 'workspace' && 'h-full min-h-0 min-w-0 gap-0',
				className,
			)}
			{...props}
		/>
	);
}

function TabsBar({ className, ...props }: ComponentProps<'div'>) {
	return (
		<div
			data-slot='tabs-bar'
			className={cn(
				'flex h-[35px] min-w-0 shrink-0 items-stretch overflow-hidden border-b border-border bg-background',
				className,
			)}
			{...props}
		/>
	);
}

const tabsListVariants = cva(
	'group/tabs-list inline-flex w-fit items-center justify-center rounded-lg p-[3px] text-muted-foreground group-data-vertical/tabs:h-fit group-data-vertical/tabs:flex-col data-[variant=line]:rounded-none',
	{
		variants: {
			variant: {
				default: 'bg-muted group-data-horizontal/tabs:h-8',
				line: 'gap-1 bg-transparent group-data-horizontal/tabs:h-8',
				workspace:
					'h-full w-fit max-w-full min-w-0 shrink justify-start gap-0 overflow-hidden rounded-none bg-transparent p-0 text-foreground',
			},
		},
		defaultVariants: {
			variant: 'default',
		},
	},
);

function TabsList({
	className,
	variant = 'default',
	...props
}: TabsPrimitive.List.Props & VariantProps<typeof tabsListVariants>) {
	return (
		<TabsPrimitive.List
			data-slot='tabs-list'
			data-variant={variant}
			className={cn(tabsListVariants({ variant }), className)}
			{...props}
		/>
	);
}

function TabsItem({ className, ...props }: ComponentProps<'div'>) {
	return (
		<div
			data-slot='tabs-item'
			className={cn(
				'relative grid h-full w-[200px] min-w-0 max-w-[200px] shrink basis-[200px]',
				className,
			)}
			{...props}
		/>
	);
}

function TabsDivider({ className, ...props }: ComponentProps<'div'>) {
	return (
		<div
			aria-hidden='true'
			data-slot='tabs-divider'
			className={cn('h-full w-px shrink-0 bg-border', className)}
			{...props}
		/>
	);
}

function TabsTrigger({ className, ...props }: TabsPrimitive.Tab.Props) {
	return (
		<TabsPrimitive.Tab
			data-slot='tabs-trigger'
			className={cn(
				"relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-1.5 py-0.5 text-sm font-medium whitespace-nowrap text-foreground/60 transition-all group-data-vertical/tabs:w-full group-data-vertical/tabs:justify-start hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50 has-data-[icon=inline-end]:pr-1 has-data-[icon=inline-start]:pl-1 aria-disabled:pointer-events-none aria-disabled:opacity-50 dark:text-muted-foreground dark:hover:text-foreground group-data-[variant=default]/tabs-list:data-active:shadow-sm group-data-[variant=line]/tabs-list:data-active:shadow-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
				'group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-active:bg-transparent dark:group-data-[variant=line]/tabs-list:data-active:border-transparent dark:group-data-[variant=line]/tabs-list:data-active:bg-transparent',
				'data-active:bg-background data-active:text-foreground dark:data-active:border-input dark:data-active:bg-input/30 dark:data-active:text-foreground',
				'after:absolute after:bg-foreground after:opacity-0 after:transition-opacity group-data-horizontal/tabs:after:inset-x-0 group-data-horizontal/tabs:after:bottom-[-5px] group-data-horizontal/tabs:after:h-0.5 group-data-vertical/tabs:after:inset-y-0 group-data-vertical/tabs:after:-right-1 group-data-vertical/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-active:after:opacity-100',
				'group-data-[variant=workspace]/tabs-list:h-full group-data-[variant=workspace]/tabs-list:min-w-0 group-data-[variant=workspace]/tabs-list:max-w-[200px] group-data-[variant=workspace]/tabs-list:basis-0 group-data-[variant=workspace]/tabs-list:grow group-data-[variant=workspace]/tabs-list:justify-start group-data-[variant=workspace]/tabs-list:overflow-hidden group-data-[variant=workspace]/tabs-list:rounded-none group-data-[variant=workspace]/tabs-list:border-0 group-data-[variant=workspace]/tabs-list:bg-transparent group-data-[variant=workspace]/tabs-list:py-0 group-data-[variant=workspace]/tabs-list:pr-8 group-data-[variant=workspace]/tabs-list:pl-3 group-data-[variant=workspace]/tabs-list:font-normal group-data-[variant=workspace]/tabs-list:shadow-none group-data-[variant=workspace]/tabs-list:hover:bg-muted group-data-[variant=workspace]/tabs-list:hover:text-foreground/60 group-data-[variant=workspace]/tabs-list:data-active:bg-[var(--ob-sys-color-surface-selected)] group-data-[variant=workspace]/tabs-list:data-active:hover:bg-[var(--ob-sys-color-surface-selected)] group-data-[variant=workspace]/tabs-list:data-active:hover:text-foreground group-data-[variant=workspace]/tabs-list:data-active:shadow-none',
				className,
			)}
			{...props}
		/>
	);
}

function TabsContent({ className, ...props }: TabsPrimitive.Panel.Props) {
	return (
		<TabsPrimitive.Panel
			data-slot='tabs-content'
			className={cn(
				'flex-1 text-sm outline-none group-data-[variant=workspace]/tabs:min-h-0 group-data-[variant=workspace]/tabs:overflow-hidden',
				className,
			)}
			{...props}
		/>
	);
}

export {
	Tabs,
	TabsBar,
	TabsContent,
	TabsDivider,
	TabsItem,
	TabsList,
	TabsTrigger,
	tabsListVariants,
};
