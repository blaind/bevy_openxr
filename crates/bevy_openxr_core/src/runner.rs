use bevy::app::App;
use bevy::utils::Instant;

pub(crate) fn xr_runner(mut app: App) {
    let mut frame = 0;

    let print_every = 20;
    let mut durations = Vec::with_capacity(print_every);
    loop {
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
        }

        frame += 1;
    }
}
