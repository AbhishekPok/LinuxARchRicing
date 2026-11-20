mod auxiliary;
mod backlight;
mod edge_activation;
mod feedback;
mod island;
mod lock;
mod theme;
mod watchdog;

use amane::App;

fn main() {
    backlight::start();
    watchdog::start();

    App::new()
        .font("JetBrainsMono Nerd Font")
        .ipc("brightness", backlight::brightness)
        .ipc("volume", island::volume)
        .ipc("media", island::media)
        .ipc("island", island::toggle)
        .ipc("bar", island::bar::command)
        .ipc("activity", island::activity::command)
        .ipc("power", island::power::command)
        .ipc("popup", island::popups::command)
        .ipc("notifications", island::notices::command)
        .ipc("awake", island::awake::command)
        .ipc("settings", island::settings::command)
        .ipc("auxiliary", auxiliary::command)
        .ipc("edge", edge_activation::command)
        .ipc("lock", lock::command)
        // The island goes on every monitor; the auxiliary overlay (search,
        // screenshot, session) stays a single window on the focused one.
        // the menu bar first, so the island's notch stacks above it
        .window_per_monitor(island::bar::view_per_monitor)
        .window_per_monitor(island::view_per_monitor)
        .window_per_monitor(auxiliary::view_per_monitor)
        // Last of the per-monitor windows, so the veil that lifts after
        // unlocking is stacked over the island and the auxiliary overlay.
        .window_per_monitor(lock::veil)
        // the lock screen on every monitor while locked, see lock.rs
        .lock(lock::view)
        .run();
}
