use core::fmt::Write;
use reducto::{App, Reducer, TextView, View};

#[derive(Clone, Debug, Default, PartialEq)]
struct TodoState {
    todos: heapless::Vec<heapless::String<32>, 8>,
}

enum TodoAction {
    Add(heapless::String<32>),
    Delete(usize),
}

struct TodoReducer;

impl Reducer for TodoReducer {
    type State = TodoState;
    type Action = TodoAction;

    fn reduce(state: &TodoState, action: TodoAction) -> TodoState {
        let mut next = state.clone();
        match action {
            TodoAction::Add(text) if text.is_empty() => {}
            TodoAction::Add(text) => {
                next.todos.push(text).ok();
            }
            TodoAction::Delete(index) if index < next.todos.len() => {
                next.todos.remove(index);
            }
            TodoAction::Delete(_) => {}
        }
        next
    }
}

struct TodoView(TextView<256>);

impl View for TodoView {
    type State = TodoState;

    fn render(&mut self, state: &TodoState) {
        self.0.clear();
        for todo in &state.todos {
            writeln!(self.0.buffer_mut(), "- {todo}").ok();
        }
    }
}

#[test]
fn todo_workflow_uses_pure_reducer() {
    let mut app: App<TodoReducer, TodoView> =
        App::new(TodoView(TextView::new()), TodoState::default());
    let mut text = heapless::String::new();
    text.push_str("Build firmware").unwrap();
    assert!(app.dispatch(TodoAction::Add(text)));
    assert!(app.view().0.contains("Build firmware"));
    assert!(app.dispatch(TodoAction::Delete(0)));
    assert!(!app.view().0.contains("Build firmware"));
    assert!(!app.dispatch(TodoAction::Delete(0)));
}
