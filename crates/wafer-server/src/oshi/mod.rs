pub mod driver;
pub mod hardware;
pub mod software;
pub mod system_info;
pub mod util;

use crate::util::java_throwable::JavaThrowable;

pub type OshiResult<T> = Result<T, JavaThrowable>;
