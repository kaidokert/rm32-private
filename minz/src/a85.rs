//! Ascii85 encoder — now lives in `minz-core` (host-tested); this
//! module re-exports it so existing `minz::a85` call sites keep
//! working.

pub use minz_core::a85::*;
