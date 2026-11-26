//! doto-skills: nhiều nguồn Skills (thư mục local + GitHub), kiểm tra an toàn, ánh xạ sang provider.
pub mod github;
pub mod github_url;
pub mod mapping;
pub mod scanner;
pub mod service;

pub use service::{Skill, SkillDetail, SkillError, SkillService, SkillSource, UpdateCheck, GITHUB_TOKEN_KEY};
