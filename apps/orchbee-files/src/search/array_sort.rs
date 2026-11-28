//! `Array.prototype.sort` as V8 runs it: the TimSort of `third_party/v8/builtins/array-sort.tq`.
//!
//! VS Code sorts Quick Open's matches with it, and its comparison treats two labels that differ
//! only in case as equal while their descriptions still order them, so it is not a consistent
//! order. Only the same runs, insertions, gallops and merges then leave files where VS Code leaves
//! them, and unlike the standard library's sorts this one never panics on such an order.

use std::cmp::Ordering;

/// `kMinGallopWins`: how many wins in a row start galloping.
const MIN_GALLOP: usize = 7;

/// Sorts `items` in place as V8's `Array.prototype.sort` does with `compare` as its comparison.
pub(super) fn sort<T: Copy>(items: &mut [T], mut compare: impl FnMut(&T, &T) -> Ordering) {
    let length = items.len();
    if length < 2 {
        return;
    }
    let mut sorter = Sorter {
        compare: &mut compare,
        runs: Vec::new(),
        min_gallop: MIN_GALLOP,
    };
    let min_run = min_run_length(length);
    let (mut low, mut remaining) = (0, length);
    while remaining != 0 {
        let mut run = sorter.count_and_make_run(items, low, low + remaining);
        if run < min_run {
            let forced = min_run.min(remaining);
            sorter.binary_insertion_sort(items, low, low + run, low + forced);
            run = forced;
        }
        sorter.runs.push((low, run));
        sorter.merge_collapse(items);
        low += run;
        remaining -= run;
    }
    sorter.merge_force_collapse(items);
}

/// `ComputeMinRunLength`.
fn min_run_length(mut length: usize) -> usize {
    let mut odd = 0;
    while length >= 64 {
        odd |= length & 1;
        length >>= 1;
    }
    length + odd
}

/// How a merge ends: the rest of the temporary copy in place, or run B's rest moved down and the
/// last of run A after it (for a merge from the top, the first of run B before run A's rest).
enum End {
    Succeed,
    CopyRest,
}

struct Sorter<'a, T> {
    compare: &'a mut dyn FnMut(&T, &T) -> Ordering,
    /// The pending runs, as base and length.
    runs: Vec<(usize, usize)>,
    min_gallop: usize,
}

impl<T: Copy> Sorter<'_, T> {
    fn less(&mut self, a: &T, b: &T) -> bool {
        (self.compare)(a, b) == Ordering::Less
    }

    /// `CountAndMakeRun`: the length of the run at `low`, reversed when it strictly descends.
    fn count_and_make_run(&mut self, items: &mut [T], low: usize, high: usize) -> usize {
        let next = low + 1;
        if next == high {
            return 1;
        }
        let mut run = 2;
        let descending = self.less(&items[next], &items[low]);
        let mut previous = items[next];
        for &current in &items[next + 1..high] {
            if self.less(&current, &previous) != descending {
                break;
            }
            previous = current;
            run += 1;
        }
        if descending {
            items[low..low + run].reverse();
        }
        run
    }

    /// `BinaryInsertionSort`: `items[low..start]` is sorted; each later element up to `high` goes
    /// after the last one it is not less than.
    fn binary_insertion_sort(&mut self, items: &mut [T], low: usize, start: usize, high: usize) {
        let first = if low == start { start + 1 } else { start };
        for start in first..high {
            let pivot = items[start];
            let (mut left, mut right) = (low, start);
            while left < right {
                let middle = left + ((right - left) >> 1);
                if self.less(&pivot, &items[middle]) {
                    right = middle;
                } else {
                    left = middle + 1;
                }
            }
            items.copy_within(left..start, left + 1);
            items[left] = pivot;
        }
    }

    /// `RunInvariantEstablished`: run `n - 2` is longer than runs `n - 1` and `n` together.
    fn invariant(&self, n: usize) -> bool {
        n < 2 || self.runs[n - 2].1 > self.runs[n - 1].1 + self.runs[n].1
    }

    /// `MergeCollapse`.
    fn merge_collapse(&mut self, items: &mut [T]) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if !self.invariant(n + 1) || !self.invariant(n) {
                if self.runs[n - 1].1 < self.runs[n + 1].1 {
                    n -= 1;
                }
                self.merge_at(items, n);
            } else if self.runs[n].1 <= self.runs[n + 1].1 {
                self.merge_at(items, n);
            } else {
                break;
            }
        }
    }

    /// `MergeForceCollapse`.
    fn merge_force_collapse(&mut self, items: &mut [T]) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if n > 0 && self.runs[n - 1].1 < self.runs[n + 1].1 {
                n -= 1;
            }
            self.merge_at(items, n);
        }
    }

    /// `MergeAt`: merges runs `i` and `i + 1`.
    fn merge_at(&mut self, items: &mut [T], i: usize) {
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];
        self.runs[i].1 = length_a + length_b;
        self.runs.remove(i + 1);
        let key = items[base_b];
        let skipped = self.gallop_right(items, &key, base_a, length_a, 0);
        base_a += skipped;
        length_a -= skipped;
        if length_a == 0 {
            return;
        }
        let key = items[base_a + length_a - 1];
        length_b = self.gallop_left(items, &key, base_b, length_b, length_b - 1);
        if length_b == 0 {
            return;
        }
        if length_a <= length_b {
            self.merge_low(items, base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(items, base_a, length_a, base_b, length_b);
        }
    }

    /// `GallopLeft`: where `key` goes in the sorted `array[base..base + length]`, before any
    /// element equal to it, searched outward from `hint`.
    fn gallop_left(
        &mut self,
        array: &[T],
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
    ) -> usize {
        let (mut last, mut offset) = (0, 1);
        let (start, end) = if self.less(&array[base + hint], key) {
            let most = length - hint;
            while offset < most {
                if !self.less(&array[base + hint + offset], key) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(most);
            (hint + last + 1, hint + offset)
        } else {
            let most = hint + 1;
            while offset < most {
                if self.less(&array[base + hint - offset], key) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(most);
            (hint + 1 - offset, hint - last)
        };
        let (mut start, mut end) = (start, end);
        while start < end {
            let middle = start + ((end - start) >> 1);
            if self.less(&array[base + middle], key) {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        end
    }

    /// `GallopRight`: where `key` goes in the sorted `array[base..base + length]`, after any
    /// element equal to it, searched outward from `hint`.
    fn gallop_right(
        &mut self,
        array: &[T],
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
    ) -> usize {
        let (mut last, mut offset) = (0, 1);
        let (start, end) = if self.less(key, &array[base + hint]) {
            let most = hint + 1;
            while offset < most {
                if !self.less(key, &array[base + hint - offset]) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(most);
            (hint + 1 - offset, hint - last)
        } else {
            let most = length - hint;
            while offset < most {
                if self.less(key, &array[base + hint + offset]) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
            }
            offset = offset.min(most);
            (hint + last + 1, hint + offset)
        };
        let (mut start, mut end) = (start, end);
        while start < end {
            let middle = start + ((end - start) >> 1);
            if self.less(key, &array[base + middle]) {
                end = middle;
            } else {
                start = middle + 1;
            }
        }
        end
    }

    /// `MergeLow`: merges run A, the shorter, from the bottom, through a copy of it.
    fn merge_low(
        &mut self,
        items: &mut [T],
        base_a: usize,
        mut length_a: usize,
        base_b: usize,
        mut length_b: usize,
    ) {
        let temp = items[base_a..base_a + length_a].to_vec();
        let (mut dest, mut cursor_temp, mut cursor_b) = (base_a, 0, base_b);
        items[dest] = items[cursor_b];
        dest += 1;
        cursor_b += 1;
        let end = 'merge: {
            length_b -= 1;
            if length_b == 0 {
                break 'merge End::Succeed;
            }
            if length_a == 1 {
                break 'merge End::CopyRest;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    if self.less(&items[cursor_b], &temp[cursor_temp]) {
                        items[dest] = items[cursor_b];
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        items[dest] = temp[cursor_temp];
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 1 {
                            break 'merge End::CopyRest;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;
                    let key = items[cursor_b];
                    wins_a = self.gallop_right(&temp, &key, cursor_temp, length_a, 0);
                    if wins_a > 0 {
                        items[dest..dest + wins_a]
                            .copy_from_slice(&temp[cursor_temp..cursor_temp + wins_a]);
                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;
                        if length_a == 1 {
                            break 'merge End::CopyRest;
                        }
                        // Only an inconsistent order empties run A here.
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    items[dest] = items[cursor_b];
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;
                    if length_b == 0 {
                        break 'merge End::Succeed;
                    }
                    let key = temp[cursor_temp];
                    wins_b = self.gallop_left(items, &key, cursor_b, length_b, 0);
                    if wins_b > 0 {
                        items.copy_within(cursor_b..cursor_b + wins_b, dest);
                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    items[dest] = temp[cursor_temp];
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;
                    if length_a == 1 {
                        break 'merge End::CopyRest;
                    }
                }
                // Leaving galloping makes it harder to start again.
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match end {
            End::Succeed => items[dest..dest + length_a]
                .copy_from_slice(&temp[cursor_temp..cursor_temp + length_a]),
            End::CopyRest => {
                items.copy_within(cursor_b..cursor_b + length_b, dest);
                items[dest + length_b] = temp[cursor_temp];
            }
        }
    }

    /// `MergeHigh`: merges run B, the shorter, from the top, through a copy of it. Each cursor is
    /// one past the next element it reads or writes.
    fn merge_high(
        &mut self,
        items: &mut [T],
        base_a: usize,
        mut length_a: usize,
        base_b: usize,
        mut length_b: usize,
    ) {
        let temp = items[base_b..base_b + length_b].to_vec();
        let (mut dest, mut cursor_temp, mut cursor_a) =
            (base_b + length_b, length_b, base_a + length_a);
        dest -= 1;
        cursor_a -= 1;
        items[dest] = items[cursor_a];
        let end = 'merge: {
            length_a -= 1;
            if length_a == 0 {
                break 'merge End::Succeed;
            }
            if length_b == 1 {
                break 'merge End::CopyRest;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    if self.less(&temp[cursor_temp - 1], &items[cursor_a - 1]) {
                        dest -= 1;
                        cursor_a -= 1;
                        items[dest] = items[cursor_a];
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    } else {
                        dest -= 1;
                        cursor_temp -= 1;
                        items[dest] = temp[cursor_temp];
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 1 {
                            break 'merge End::CopyRest;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;
                    let key = temp[cursor_temp - 1];
                    let at = self.gallop_right(items, &key, base_a, length_a, length_a - 1);
                    wins_a = length_a - at;
                    if wins_a > 0 {
                        dest -= wins_a;
                        cursor_a -= wins_a;
                        items.copy_within(cursor_a..cursor_a + wins_a, dest);
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    dest -= 1;
                    cursor_temp -= 1;
                    items[dest] = temp[cursor_temp];
                    length_b -= 1;
                    if length_b == 1 {
                        break 'merge End::CopyRest;
                    }
                    let key = items[cursor_a - 1];
                    let at = self.gallop_left(&temp, &key, 0, length_b, length_b - 1);
                    wins_b = length_b - at;
                    if wins_b > 0 {
                        dest -= wins_b;
                        cursor_temp -= wins_b;
                        items[dest..dest + wins_b]
                            .copy_from_slice(&temp[cursor_temp..cursor_temp + wins_b]);
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge End::CopyRest;
                        }
                        // Only an inconsistent order empties run B here.
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    dest -= 1;
                    cursor_a -= 1;
                    items[dest] = items[cursor_a];
                    length_a -= 1;
                    if length_a == 0 {
                        break 'merge End::Succeed;
                    }
                }
                // Leaving galloping makes it harder to start again.
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match end {
            End::Succeed => {
                items[dest - length_b..dest].copy_from_slice(&temp[..length_b]);
            }
            End::CopyRest => {
                dest -= length_a;
                cursor_a -= length_a;
                items.copy_within(cursor_a..cursor_a + length_a, dest);
                items[dest - 1] = temp[cursor_temp - 1];
            }
        }
    }
}
