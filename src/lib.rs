#[macro_use]
extern crate lazy_static;
lazy_static! {
    static ref EGUI_CTX: egui::Context = egui::Context::default();
}
mod backends;
pub mod frontends;
fn ui_fn(ctx: &egui::Context) {
    egui::Window::new("TEST")
        .resize(|r| r.auto_sized())
        .show(ctx, |ui| ui.label("other"));
}
