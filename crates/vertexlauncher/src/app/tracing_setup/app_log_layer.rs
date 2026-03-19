use tracing_subscriber::layer::{Context as LayerContext, Layer};

use crate::app::tracing_setup::{
    SharedLogWriter, current_date_time_parts, format_module_path, message_visitor::MessageVisitor,
    should_omit_module_path, write_log_line,
};

#[derive(Clone)]
pub(super) struct AppLogLayer {
    pub(super) writer: SharedLogWriter,
}

impl<S> Layer<S> for AppLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: LayerContext<'_, S>) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let (date, time) = current_date_time_parts();
        let level = meta.level().as_str();
        let module_path = format_module_path(meta.target(), meta.file());
        let message = if visitor.message.is_empty() {
            visitor.fields
        } else if visitor.fields.is_empty() {
            visitor.message
        } else {
            format!("{} {}", visitor.message, visitor.fields)
        };
        let line = if should_omit_module_path(meta.target(), &module_path) {
            format!("[{date}][{time}][{level}]: {message}")
        } else {
            format!("[{date}][{time}][{level}][{module_path}]: {message}")
        };

        launcher_ui::console::push_line(line.clone());
        write_log_line(&self.writer, &line);
    }
}
