use ratatui::style::Color;

// ============================================================================
// Background Colors
// ============================================================================

/// Main background color for most UI elements
pub const NORMAL_BG: Color = Color::Rgb(19, 19, 19);

/// Alternate background color for visual variety
pub const ALT_BG: Color = Color::Rgb(25, 25, 25);

/// Background color for selected items and header
pub const SELECTED_BG: Color = Color::Rgb(36, 36, 36);

/// Header background color (same as selected for consistency)
pub const HEADER_BG: Color = Color::Rgb(36, 36, 36);

// ============================================================================
// Text/Foreground Colors
// ============================================================================

/// Standard text color
pub const TEXT_FG: Color = Color::Rgb(200, 200, 200);

/// Header text color
pub const HEADER_FG: Color = Color::Rgb(200, 200, 200);

/// White text for high contrast
pub const TEXT_WHITE: Color = Color::White;

// ============================================================================
// Border Colors
// ============================================================================

/// Normal border color
pub const BORDER_NORMAL: Color = Color::Rgb(116, 116, 116);

/// Border color during processing/normal mode
pub const BORDER_PROCESSING: Color = Color::Rgb(116, 116, 116);

/// Border color for insert mode
pub const BORDER_INSERT: Color = Color::Rgb(165, 165, 165);

/// Border color for new task modal (green accent)
pub const BORDER_NEW: Color = Color::Green;

/// Border color for edit mode (blue accent)
pub const BORDER_EDIT: Color = Color::Blue;

/// Border color for confirmation/danger actions
pub const BORDER_DANGER: Color = Color::Red;

// ============================================================================
// Semantic Colors
// ============================================================================

/// High priority color
pub const PRIORITY_HIGH: Color = Color::Red;

/// Medium priority color
pub const PRIORITY_MEDIUM: Color = Color::Yellow;

/// Low priority color
pub const PRIORITY_LOW: Color = Color::Blue;

/// No/default priority color
pub const PRIORITY_NONE: Color = Color::Gray;

/// Color for overdue dates
pub const DATE_OVERDUE: Color = Color::Rgb(150, 80, 80);

/// Color for normal dates
pub const DATE_NORMAL: Color = Color::Rgb(100, 100, 100);

// ============================================================================
// Accent Colors
// ============================================================================

/// Yellow accent for labels and warnings
pub const ACCENT_YELLOW: Color = Color::Yellow;

// /// Cyan accent for examples and hints
// pub const ACCENT_CYAN: Color = Color::Cyan;

/// Green accent for success/confirmation
pub const ACCENT_GREEN: Color = Color::Green;

/// Red accent for errors/cancellation
pub const ACCENT_RED: Color = Color::Red;
