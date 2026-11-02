#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuation {
    Continue,
    Abort,
}

impl Continuation {
    pub fn should_abort(self) -> bool {
        self == Continuation::Abort
    }

    pub fn abort_if(abort_if: bool) -> Continuation {
        if abort_if { Continuation::Abort } else { Continuation::Continue }
    }

    pub fn continue_if(continue_if: bool) -> Continuation {
        Continuation::abort_if(!continue_if)
    }
}
