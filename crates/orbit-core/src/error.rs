use thiserror::Error;

#[derive(Debug, Error)]
pub enum VtError {
    #[error("unrecognized CSI sequence: intermediates={intermediates:?} action={action}")]
    UnrecognizedCsi {
        intermediates: Vec<u8>,
        action: char,
    },

    #[error("grid index out of bounds: ({col}, {row}) in {cols}x{rows}")]
    GridOutOfBounds {
        col: u16,
        row: u16,
        cols: u16,
        rows: u16,
    },

    #[error("invalid scroll region: top={top} bottom={bottom} rows={rows}")]
    InvalidScrollRegion { top: u16, bottom: u16, rows: u16 },
}

#[derive(Debug, Error)]
pub enum GridError {
    #[error("resize failed: requested {requested_cols}x{requested_rows}")]
    ResizeFailed {
        requested_cols: u16,
        requested_rows: u16,
    },

    #[error("alternate screen state corruption")]
    AlternateScreenCorrupt,
}
