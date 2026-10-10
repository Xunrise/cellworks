//! What DomainKnowledge tells the Map for the cells lens: the bounded
//! contexts a project documents, the code that implements each, and the
//! context map's relationships between them.

use std::path::PathBuf;

/// A project's bounded contexts and the roads between them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoundedContexts {
    pub contexts: Vec<BoundedContext>,
    pub roads: Vec<Road>,
}

/// One bounded context.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedContext {
    pub name: String,
    /// The folders that implement it, canonical, as the project's settings
    /// name them; empty where they do not, and then the folder of the same
    /// name does.
    pub code: Vec<PathBuf>,
}

/// A relationship of the context map: upstream, downstream, and its type
/// as the engineer reads it ("published language", "conformist", …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Road {
    pub upstream: String,
    pub downstream: String,
    pub kind: String,
}
