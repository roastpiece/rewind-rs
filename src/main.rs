mod app;
mod graphics;
mod gui;
mod models;

fn main() {
    env_logger::init();

    let mut app = app::App::default();
    app.run();
}



