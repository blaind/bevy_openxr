use bevy::app::App;
use bevy::app::AppExit;
use bevy::ecs::event::Events;
use bevy::ecs::event::ManualEventReader;
use bevy::utils::Instant;
use wgpu::wgpu_openxr::WGPUOpenXR;

pub(crate) fn xr_runner(mut app: App) {
    let mut frame = 0;

    let print_every = 1000;
    let mut durations = Vec::with_capacity(print_every);
    let mut app_exit_event_reader = ManualEventReader::<AppExit>::default();

    loop {

        if let Some(app_exit_events) = app.world.get_resource_mut::<Events<AppExit>>() {
            if app_exit_event_reader
                .iter(&app_exit_events)
                .next_back()
                .is_some()
            {
                println!("Exit triggered...");
                break;
            }
        }

        let start = Instant::now();
        app.update();
        durations.push(start.elapsed());

        if frame % print_every == 0 {
            let total: u128 = durations.iter().map(|d| d.as_millis()).sum();
            let average = total as f32 / durations.len() as f32;

            let fps = 1000.0 / average;
            println!(
                "[app.update()]: Previous {} frames took on average {:.2}ms per frame ({:.1} fps) ",
                print_every, average, fps
            );

            durations.clear();
        }

        frame += 1;
    }

    let wgpu_openxr = app.world.get_resource::<WGPUOpenXR>().unwrap();
    wgpu_openxr.destroy();
}
