mod model;
#[cfg(feature = "cli")]
mod reader;

pub use model::{ParsedRequest, RequestBody, RequestDefinition, RequestTemplate};
#[cfg(feature = "cli")]
pub use reader::parse_request_file;
