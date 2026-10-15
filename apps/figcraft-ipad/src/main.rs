//! iOS entry point. Host workspace checks do not initialize the native host.
#[cfg(target_os = "ios")]
mod app;

#[cfg(target_os = "ios")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("figcraft-ipad runs on iOS; use packaging/ipad/build.sh simulator");
}
