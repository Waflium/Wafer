use super::*;

#[test]
fn a_panic_message_keeps_a_java_class_name_and_gets_one_otherwise() {
    assert_eq!(JavaThrowable::from_panic("java.lang.IllegalStateException: boom").description, "java.lang.IllegalStateException: boom");
    assert_eq!(JavaThrowable::from_panic("java.lang.ArithmeticException").description, "java.lang.ArithmeticException");
    assert_eq!(JavaThrowable::from_panic("a world preset").description, "java.lang.RuntimeException: a world preset");
    assert_eq!(JavaThrowable::from_panic("").description, "java.lang.RuntimeException: ");
}

#[test]
fn prints_a_single_throwable_with_its_frames() {
    let throwable = JavaThrowable::new("java.lang.IllegalStateException: boom", vec!["a.B.c(B.java:1)", "a.B.d(B.java:2)"]);
    assert_eq!(throwable.print_stack_trace(), "java.lang.IllegalStateException: boom\n\tat a.B.c(B.java:1)\n\tat a.B.d(B.java:2)\n");
}

#[test]
fn elides_the_frames_a_cause_shares_with_its_enclosing_trace() {
    let cause = JavaThrowable::new("java.io.IOException: inner", vec!["a.B.inner(B.java:9)", "a.B.shared(B.java:3)", "a.B.main(B.java:4)"]);
    let throwable = JavaThrowable::new("java.lang.IllegalStateException: outer", vec!["a.B.outer(B.java:8)", "a.B.shared(B.java:3)", "a.B.main(B.java:4)"]).with_cause(cause);
    assert_eq!(
        throwable.print_stack_trace(),
        "java.lang.IllegalStateException: outer\n\tat a.B.outer(B.java:8)\n\tat a.B.shared(B.java:3)\n\tat a.B.main(B.java:4)\nCaused by: java.io.IOException: inner\n\tat a.B.inner(B.java:9)\n\t... 2 more\n"
    );
}

#[test]
fn keeps_every_frame_when_nothing_is_shared() {
    let cause = JavaThrowable::new("java.io.IOException: inner", vec!["a.B.inner(B.java:9)"]);
    let throwable = JavaThrowable::new("java.lang.IllegalStateException: outer", vec!["a.B.outer(B.java:8)"]).with_cause(cause);
    assert_eq!(throwable.print_stack_trace(), "java.lang.IllegalStateException: outer\n\tat a.B.outer(B.java:8)\nCaused by: java.io.IOException: inner\n\tat a.B.inner(B.java:9)\n");
}

#[test]
fn chains_causes_against_their_direct_enclosing_trace() {
    let root = JavaThrowable::new("c.Root: r", vec!["x.y(Y.java:1)", "x.z(Z.java:2)"]);
    let middle = JavaThrowable::new("c.Middle: m", vec!["x.m(M.java:5)", "x.z(Z.java:2)"]).with_cause(root);
    let top = JavaThrowable::new("c.Top: t", vec!["x.t(T.java:7)"]).with_cause(middle);
    assert_eq!(
        top.print_stack_trace(),
        "c.Top: t\n\tat x.t(T.java:7)\nCaused by: c.Middle: m\n\tat x.m(M.java:5)\n\tat x.z(Z.java:2)\nCaused by: c.Root: r\n\tat x.y(Y.java:1)\n\t... 1 more\n"
    );
}
