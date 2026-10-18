//! Synchronous main-thread geometry values; native objects never leave this call.
use objc2::MainThreadMarker;
use objc2_ui_kit::{UIApplication, UISceneActivationState, UIWindowScene};

/// Keyboard bounds in window points, or None when the guide is offscreen.
pub fn keyboard_bounds() -> Result<Option<[f32; 4]>, String> {
    let mtm = MainThreadMarker::new().ok_or("keyboard geometry requires the main thread")?;
    for scene in UIApplication::sharedApplication(mtm).connectedScenes().iter() {
        if scene.activationState() != UISceneActivationState::ForegroundActive {
            continue;
        }
        let Ok(scene) = scene.downcast::<UIWindowScene>() else { continue };
        for window in scene.windows().iter() {
            if !window.isKeyWindow() {
                continue;
            }
            let view =
                window.rootViewController().ok_or("keyboard geometry has no root controller")?.view().ok_or("keyboard geometry has no root view")?;
            let guide = view.keyboardLayoutGuide();
            guide.setFollowsUndockedKeyboard(true);
            guide.setUsesBottomSafeArea(false);
            view.layoutIfNeeded();
            let frame = view.convertRect_toView(guide.layoutFrame(), Some(&window));
            let bounds = [
                frame.origin.x as f32,
                frame.origin.y as f32,
                (frame.origin.x + frame.size.width) as f32,
                (frame.origin.y + frame.size.height) as f32,
            ];
            if !bounds.iter().all(|v| v.is_finite()) {
                return Err("keyboard layout guide returned nonfinite bounds".into());
            }
            return Ok((bounds[2] > bounds[0] && bounds[3] > bounds[1]).then_some(bounds));
        }
    }
    Err("keyboard geometry has no foreground key window".into())
}
