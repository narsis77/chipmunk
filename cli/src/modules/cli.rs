use super::{Kind, Manager};
use crate::{location::get_root, modules::TestCommand, Target};
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Clone, Debug)]
/// Represents the path `cli`
pub struct Module {}

impl Module {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Manager for Module {
    fn owner(&self) -> Target {
        Target::Cli
    }
    fn kind(&self) -> Kind {
        Kind::Rs
    }
    fn cwd(&self) -> PathBuf {
        get_root().join("cli")
    }
    fn deps(&self) -> Vec<Target> {
        vec![]
    }
    fn test_cmds(&self) -> Vec<TestCommand> {
        vec![TestCommand::new(
            "cargo +stable test --color always".into(),
            self.cwd(),
            None,
        )]
    }
}
