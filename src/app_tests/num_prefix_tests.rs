use super::support::*;
use super::*;

#[test]
fn push_digit_builds_number() {
    let mut app = App::new(make_config());
    app.push_digit(1);
    app.push_digit(2);
    app.push_digit(3);
    assert_eq!(app.num_prefix, 123);
}

#[test]
fn consume_prefix_returns_one_when_zero() {
    let mut app = App::new(make_config());
    assert_eq!(app.consume_prefix(), 1);
    assert_eq!(app.num_prefix, 0);
}

#[test]
fn consume_prefix_returns_value_and_resets() {
    let mut app = App::new(make_config());
    app.push_digit(5);
    assert_eq!(app.consume_prefix(), 5);
    assert_eq!(app.num_prefix, 0);
}
