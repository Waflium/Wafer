use super::file_fix_exception::FileFixException;

#[derive(Debug)]
pub struct CanceledFileFixException;

impl CanceledFileFixException {
    pub fn new() -> FileFixException {
        FileFixException::Canceled(CanceledFileFixException)
    }
}
