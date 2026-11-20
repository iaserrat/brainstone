//! Agent Skills discovery and progressive-disclosure tools.

#![deny(missing_docs)]
mod admission;
mod catalog;
mod catalog_load;
mod catalog_registry;
mod diagnostic;
mod diagnostic_display;
mod error;
mod error_code;
mod error_display;
mod explicit;
mod frontmatter;
mod raw;
mod resource_read;
mod resource_tool;
mod resources;
mod skill;
mod skill_activate;
mod skill_content;
mod skill_tool;

pub use admission::SkillAdmission;
pub use catalog::Catalog;
pub use diagnostic::Diagnostic;
pub use error::Error;
pub use skill::Skill;
