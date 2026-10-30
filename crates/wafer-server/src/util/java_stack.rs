use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

thread_local! {
    static STACK: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

static INTERNED: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

pub fn intern(frame: String) -> &'static str {
    let mut interned = INTERNED.lock().unwrap_or_else(PoisonError::into_inner);
    let set = interned.get_or_insert_with(HashSet::new);
    match set.get(frame.as_str()) {
        Some(existing) => existing,
        None => {
            let leaked: &'static str = Box::leak(frame.into_boxed_str());
            set.insert(leaked);
            leaked
        }
    }
}

pub fn frame(method: &str, file: &str, line: u32) -> &'static str {
    intern(format!("{method}({file}:{line})"))
}

pub struct StackScope(());

impl Drop for StackScope {
    fn drop(&mut self) {
        STACK.with(|stack| stack.borrow_mut().pop());
    }
}

pub fn enter(frame: &'static str) -> StackScope {
    STACK.with(|stack| stack.borrow_mut().push(frame));
    StackScope(())
}

pub fn enter_all(frames_top_first: &[&'static str]) -> Vec<StackScope> {
    frames_top_first.iter().rev().map(|frame| enter(frame)).collect()
}

pub fn replace_top(frame: &'static str) {
    STACK.with(|stack| {
        if let Some(top) = stack.borrow_mut().last_mut() {
            *top = frame;
        }
    });
}

pub fn capture(top_first: &[&'static str]) -> Vec<&'static str> {
    STACK.with(|stack| top_first.iter().copied().chain(stack.borrow().iter().rev().copied()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_frames_start_at_the_innermost_call() {
        let _outer = enter("a.Outer.run(Outer.java:1)");
        let _inner = enter("a.Inner.run(Inner.java:2)");
        replace_top("a.Inner.run(Inner.java:3)");
        assert_eq!(capture(&["a.Top.run(Top.java:4)"]), vec!["a.Top.run(Top.java:4)", "a.Inner.run(Inner.java:3)", "a.Outer.run(Outer.java:1)"]);
    }

    #[test]
    fn interning_returns_the_same_string() {
        assert!(std::ptr::eq(frame("a.B.c", "B.java", 7), frame("a.B.c", "B.java", 7)));
    }
}
