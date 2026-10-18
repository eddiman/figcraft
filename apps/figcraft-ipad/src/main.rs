//! iOS entry point. Host workspace checks do not initialize the native host.
#[cfg(target_os = "ios")]
mod app;
#[cfg(target_os = "ios")]
mod clipboard;
#[cfg(any(target_os = "ios", test))]
mod documents;
#[cfg(target_os = "ios")]
mod lifecycle;
#[cfg(any(target_os = "ios", test))]
mod recovery;
#[cfg(target_os = "ios")]
mod services;

#[cfg(target_os = "ios")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("figcraft-ipad runs on iOS; use packaging/ipad/build.sh simulator");
}
