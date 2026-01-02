pub mod ast_json;
pub mod backend;
pub mod capabilities;
pub mod code_actions;
pub mod completion;
pub mod diagnostics;
pub mod formatting;
pub mod hover;
pub mod line_index;
pub mod navigation;
pub mod semantic_index;
// MCP module is NOT part of the library, it's a separate binary.
// But we might want to share MCP types if we were doing in-process, but here we are doing separate bin.
