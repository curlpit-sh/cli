mod model;
mod reader;

pub use model::{ParsedRequest, RequestBody, RequestDefinition, RequestTemplate};
pub use reader::parse_request_file;
