//! Helm Notes — application-level capture and intelligence for Kenneth's vault.
//!
//! This crate owns the "smart capture" logic: given a URL, file, or image,
//! it figures out what kind of content it is, extracts structured data using
//! organism-intelligence, formats a rich Markdown note, and writes it to the
//! vault via organism-notes.
//!
//! Both the desktop app (Tauri) and CLI call into this library.
//!
//! # Architecture
//!
//! ```text
//! helm-notes (this crate — application logic)
//!     ├── organism-intelligence (extraction providers)
//!     └── organism-notes (vault engine)
//! ```

pub mod capture;

pub use capture::{CaptureReport, CaptureRequest, capture};
