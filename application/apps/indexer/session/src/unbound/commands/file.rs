use super::{CommandOutcome, CommandOutcome::Finished};
use crate::events::{ComputationError, ComputationError::OperationNotSupported};
use std::path::Path;
use file_tools::is_binary;

pub fn is_file_binary(
    file_path: &Path
) -> Result<CommandOutcome<bool>, ComputationError> {
    match is_binary(file_path) {
        Ok(is_binary) => Ok(Finished(is_binary)),
        Err(err) => Err(OperationNotSupported(err.to_string()))
    }
}
