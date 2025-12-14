/// Events that can occur in the monitor TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEvent {
    /// Quit the application
    Quit,
    /// Toggle help overlay
    ToggleHelp,
    /// Switch to next tab/section
    NextTab,
    /// Switch to previous tab/section
    PrevTab,
    /// Toggle process sort mode
    ToggleProcessSort,
    /// Toggle process tree view
    ToggleProcessTree,
    /// Navigate process list up
    ProcessUp,
    /// Navigate process list down
    ProcessDown,
    /// No action
    None,
}
