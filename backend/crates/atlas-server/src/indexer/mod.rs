pub(crate) mod batch;
pub(crate) mod copy;
pub(crate) mod fetcher;
#[allow(clippy::module_inception)]
pub mod indexer;
pub mod metadata;

pub use indexer::Indexer;
pub use metadata::MetadataFetcher;
