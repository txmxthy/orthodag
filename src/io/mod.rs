//! Getting graphs in and drawings out.
//!
//! The library proper never parses anything: a caller builds a [`Graph`] and
//! hands it over. These are the conveniences for when the graph arrived as text
//! from somewhere else, and each is behind a feature so nobody pays for one
//! they do not use.
//!
//! [`Graph`]: crate::Graph

#[cfg(feature = "mermaid")]
pub mod mermaid_in;
