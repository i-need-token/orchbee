use super::*;

#[test]
fn copied_text_budget_counts_unique_ranges_and_fails_before_overallocation() {
    let bytes = include_bytes!(
        "../../../../../../schemas/terminal/v1/fixtures/ghostty-render-row-window.bin"
    );
    let view = decode_terminal_render_row_window(bytes).unwrap();
    let mut unique = std::collections::HashSet::new();
    let total: usize = view
        .projection()
        .rows()
        .flat_map(|r| r.cells())
        .filter_map(|cell| {
            unique
                .insert((cell.text.as_ptr() as usize, cell.text.len()))
                .then_some(cell.text.len())
        })
        .sum();
    assert!(total > 0);
    assert!(materialize(&view, total).is_ok());
    assert_eq!(
        materialize(&view, total - 1).unwrap_err(),
        ViewportWindowFailure::MemoryLimit
    );
}
