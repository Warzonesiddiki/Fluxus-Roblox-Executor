/// # Driver — High-Level Macro Sequence Execution
///
/// Orchestrates pre-recorded navigation routes between game hubs.
/// Each route is a sequence of humanised micro-actions executed by
/// the `HumanizedInput` engine.

pub mod macro_sequences;

pub use macro_sequences::{MacroRoute, MacroStep};

/// Execute a route from start to finish.
pub fn follow_route(route: &MacroRoute, input: &mut wsx_input::HumanizedInput) {
    route.execute(input);
}
