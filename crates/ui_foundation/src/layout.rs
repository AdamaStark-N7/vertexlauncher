use egui::Ui;

#[derive(Clone, Copy, Debug)]
pub struct UiMetrics {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub compact: bool,
}

impl UiMetrics {
    pub fn from_ui(ui: &Ui, compact_threshold: f32) -> Self {
        let viewport = ui.ctx().input(|i| i.content_rect());
        Self {
            viewport_width: viewport.width().max(1.0),
            viewport_height: viewport.height().max(1.0),
            compact: is_compact_width(viewport.width(), compact_threshold),
        }
    }

    pub fn popup_width(self, min: f32, max: f32, margin: f32) -> f32 {
        popup_width(self.viewport_width, min, max, margin)
    }

    pub fn scaled_width(self, fraction: f32, min: f32, max: f32) -> f32 {
        (self.viewport_width * fraction).clamp(min, max)
    }

    pub fn scaled_height(self, fraction: f32, min: f32, max: f32) -> f32 {
        (self.viewport_height * fraction).clamp(min, max)
    }

    pub fn columns(
        self,
        available_width: f32,
        min_column_width: f32,
        gap: f32,
        max_columns: usize,
    ) -> (usize, f32) {
        responsive_columns(available_width, min_column_width, gap, max_columns)
    }
}

pub fn is_compact_width(width: f32, threshold: f32) -> bool {
    width < threshold
}

pub fn popup_width(viewport_width: f32, min: f32, max: f32, margin: f32) -> f32 {
    let available = (viewport_width - margin * 2.0).max(1.0);
    if available <= min {
        available
    } else {
        available.clamp(min, max)
    }
}

pub fn responsive_columns(
    available_width: f32,
    min_column_width: f32,
    gap: f32,
    max_columns: usize,
) -> (usize, f32) {
    let max_columns = max_columns.max(1);
    let mut columns = 1;
    for candidate in 1..=max_columns {
        let required_width =
            (min_column_width * candidate as f32) + (gap * (candidate.saturating_sub(1) as f32));
        if required_width <= available_width {
            columns = candidate;
        }
    }
    let width =
        ((available_width - gap * (columns.saturating_sub(1) as f32)) / columns as f32).max(1.0);
    (columns, width)
}
