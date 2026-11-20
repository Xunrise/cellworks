//! The Sources bounded context: the loaders for tickets (Jira, Cockpit, GitHub
//! issues, Linear), infrastructure and data projects.
//!
//! Defined by docs/domain/contexts/Sources/Sources.md. Nothing is exposed yet;
//! this crate is the seam the Sources stories grow into.

pub mod pull_requests;
pub mod stories;
pub mod tickets;
