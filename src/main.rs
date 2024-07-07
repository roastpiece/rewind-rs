mod app;
mod graphics;
mod gui;

fn main() {
    env_logger::init();

    let mut app = app::App::default();
    app.run();
}



