use ratatui::{backend::TestBackend, Terminal};

/// Setup a test terminal with the given width and height.
///
/// # Panics
///
/// Panics if the terminal cannot be created.
pub fn setup_test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let terminal = Terminal::new(backend).unwrap();
    terminal
}
