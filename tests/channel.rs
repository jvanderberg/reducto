//! Tests for ActionChannel (requires `embassy` feature)
//!
//! Run with: cargo test --features embassy

#![cfg(feature = "embassy")]

use reducto::ActionChannel;

#[derive(Debug, Clone, PartialEq)]
enum TestAction {
    Increment,
    Decrement,
    SetValue(i32),
}

#[test]
fn channel_can_be_created() {
    static CHANNEL: ActionChannel<TestAction, 8> = ActionChannel::new();
    // Just verify it compiles and can be a static
    assert!(CHANNEL.is_empty());
}

#[test]
fn channel_try_send_and_try_receive() {
    static CHANNEL: ActionChannel<TestAction, 4> = ActionChannel::new();

    // Send some actions
    assert!(CHANNEL.try_send(TestAction::Increment).is_ok());
    assert!(CHANNEL.try_send(TestAction::SetValue(42)).is_ok());
    assert!(CHANNEL.try_send(TestAction::Decrement).is_ok());

    // Receive them in order
    assert_eq!(CHANNEL.try_receive().ok(), Some(TestAction::Increment));
    assert_eq!(CHANNEL.try_receive().ok(), Some(TestAction::SetValue(42)));
    assert_eq!(CHANNEL.try_receive().ok(), Some(TestAction::Decrement));

    // Channel is now empty
    assert!(CHANNEL.is_empty());
    assert!(CHANNEL.try_receive().is_err());
}

#[test]
fn channel_respects_capacity() {
    static CHANNEL: ActionChannel<TestAction, 2> = ActionChannel::new();

    // Fill the channel
    assert!(CHANNEL.try_send(TestAction::Increment).is_ok());
    assert!(CHANNEL.try_send(TestAction::Decrement).is_ok());

    // Channel is full
    assert!(CHANNEL.is_full());
    assert!(CHANNEL.try_send(TestAction::Increment).is_err());

    // Drain one, now we can send again
    assert!(CHANNEL.try_receive().is_ok());
    assert!(CHANNEL.try_send(TestAction::SetValue(99)).is_ok());
}

#[test]
fn channel_is_fifo() {
    static CHANNEL: ActionChannel<i32, 8> = ActionChannel::new();

    // Send 0, 1, 2, 3, 4
    for i in 0..5 {
        CHANNEL.try_send(i).unwrap();
    }

    // Receive in same order
    for i in 0..5 {
        assert_eq!(CHANNEL.try_receive().ok(), Some(i));
    }
}
