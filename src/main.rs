mod app;
mod graphics;

fn main() {
    env_logger::init();

    let mut app = app::App::default();
    app.run();
}



