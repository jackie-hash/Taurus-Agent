//! Hermes-style persistent memory system for Taurus.
//!
//! Provides a multi-file memory system with typed memories (user, project,
//! feedback, reference) and an index file (MEMORY.md) that tracks all memory
//! entries. Supports auto-memory capture from conversation via LLM.
//!
//! ## Directory structure
//!
//! ```text
//! ~/.taurus/memory/
//! ├── MEMORY.md              # Global memory index
//! ├── user_role.md           # User role / preferences
//! ├── feedback_testing.md    # Feedback on approach
//! ├── project/
//! │   └── <project-slug>/
//! │       ├── MEMORY.md      # Project-specific index
//! │       └── <memory>.md    # Individual project memories
//! └── reference/
//!     └── <topic>.md         # External resource pointers
//! ```

pub mod capture;
pub mod index;
pub mod store;
pub mod types;

pub use capture::MemoryCapture;
pub use store::MemoryStore;
pub use types::MemoryIndex;
pub use types::*;
