use super::{
    Geometry, GeometryError, GeometryField, MAXIMUM_GEOMETRY_VALUE, MAXIMUM_PIXEL_EXTENT,
    MAXIMUM_TERMINAL_CELLS,
};

#[test]
fn accepts_scalar_and_product_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    let exact_products = Geometry::new(255, 255, 257, 257)?;
    assert_eq!(
        u32::from(exact_products.columns()) * u32::from(exact_products.cell_pixel_width()),
        MAXIMUM_PIXEL_EXTENT
    );
    assert_eq!(
        u32::from(exact_products.rows()) * u32::from(exact_products.cell_pixel_height()),
        MAXIMUM_PIXEL_EXTENT
    );

    let exact_cells = Geometry::new(512, 512, 1, 1)?;
    assert_eq!(
        u32::from(exact_cells.columns()) * u32::from(exact_cells.rows()),
        MAXIMUM_TERMINAL_CELLS
    );
    assert!(Geometry::new(MAXIMUM_GEOMETRY_VALUE, 1, 15, MAXIMUM_GEOMETRY_VALUE).is_ok());
    Ok(())
}

#[test]
fn rejects_every_scalar_bound() {
    for (field, result) in [
        (GeometryField::Columns, Geometry::new(0, 1, 1, 1)),
        (GeometryField::Rows, Geometry::new(1, 0, 1, 1)),
        (GeometryField::CellPixelWidth, Geometry::new(1, 1, 0, 1)),
        (GeometryField::CellPixelHeight, Geometry::new(1, 1, 1, 0)),
        (
            GeometryField::Columns,
            Geometry::new(MAXIMUM_GEOMETRY_VALUE + 1, 1, 1, 1),
        ),
        (
            GeometryField::Rows,
            Geometry::new(1, MAXIMUM_GEOMETRY_VALUE + 1, 1, 1),
        ),
        (
            GeometryField::CellPixelWidth,
            Geometry::new(1, 1, MAXIMUM_GEOMETRY_VALUE + 1, 1),
        ),
        (
            GeometryField::CellPixelHeight,
            Geometry::new(1, 1, 1, MAXIMUM_GEOMETRY_VALUE + 1),
        ),
    ] {
        assert!(
            matches!(
                result,
                Err(GeometryError::ValueOutOfRange {
                    field: actual_field,
                    ..
                }) if actual_field == field
            ),
            "geometry field {field}"
        );
    }
}

#[test]
fn rejects_each_independent_product_overflow() {
    assert_eq!(
        Geometry::new(512, 513, 1, 1),
        Err(GeometryError::TerminalCellCountTooLarge {
            actual: 262_656,
            maximum: MAXIMUM_TERMINAL_CELLS,
        })
    );
    assert_eq!(
        Geometry::new(4096, 1, 16, 1),
        Err(GeometryError::PixelWidthTooLarge {
            actual: 65_536,
            maximum: MAXIMUM_PIXEL_EXTENT,
        })
    );
    assert_eq!(
        Geometry::new(1, 4096, 1, 16),
        Err(GeometryError::PixelHeightTooLarge {
            actual: 65_536,
            maximum: MAXIMUM_PIXEL_EXTENT,
        })
    );
}
