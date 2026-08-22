import { useLayoutEffect, useRef } from 'react';

type Callable<ThisType, Arguments extends unknown[], Result> = (
	this: ThisType,
	...args: Arguments
) => Result;

export function useMemoizedFn<ThisType, Arguments extends unknown[], Result>(
	fn: Callable<ThisType, Arguments, Result>,
): Callable<ThisType, Arguments, Result> {
	const fnRef = useRef(fn);
	useLayoutEffect(() => {
		fnRef.current = fn;
	}, [fn]);

	const memoizedFnRef = useRef<Callable<ThisType, Arguments, Result> | undefined>(undefined);
	if (memoizedFnRef.current === undefined) {
		memoizedFnRef.current = function (this: ThisType, ...args: Arguments): Result {
			return fnRef.current.apply(this, args);
		};
	}

	return memoizedFnRef.current;
}
