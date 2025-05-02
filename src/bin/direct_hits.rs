use std::env;
use std::fs;

use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::MatchState;
use tf_demo_parser::demo::parser::gamestateanalyser::GameStateAnalyser;
pub use tf_demo_parser::{Demo, DemoParser, Parse};
use tf_demo_parser::demo::data::game_state::Player;

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemo {
    header: Header,
    #[serde(flatten)]
    state: MatchState,
}

fn main() -> Result<(), MainError> {
    #[cfg(feature = "better-panic")]
    better_panic::install();

    #[cfg(feature = "trace")]
    tracing_subscriber::fmt::init();

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("1 argument required");
        return Ok(());
    }
    let path = args[1].clone();
    let file = fs::read(path)?;
    let demo = Demo::new(&file);

    let parser = DemoParser::new_all_with_analyser(demo.get_stream(), GameStateAnalyser::default());
    let (_header, state) = parser.parse()?;

    for collision in &state.collisions {
        let bruh = state.get_player(collision.target).unwrap();
        if let Some(player) = state
            .get_player(collision.target)
            .and_then(|player| player.info.as_ref())
        {
            let weapon_class = state
                .server_classes
                .get(usize::from(collision.projectile.class))
                .map(|class| class.name.as_str())
                .unwrap_or("unknown weapon");

            let shooter = state
                .players
                .iter()
                .find(|player| {
                    player
                        .weapons
                        .iter()
                        .any(|weapon| collision.projectile.launcher == *weapon)
                });
            let shooter_info = shooter.and_then(|player| player.info.as_ref());
            let mut midair = false;
            let mut flags: u16 = 0;
            if let Some(shooter) = shooter {
                midair = bruh.is_in_air();
                flags = bruh.flags;
            }

            if let Some(shooter_info) = shooter_info {
                println!(
                    "{}: {} hit by {} from {}, in air: {}, all flags: {}",
                    collision.tick, player.name, weapon_class, shooter_info.name, midair, flags
                );
            } else {
                println!(
                    "{}: {} hit by {} from unknown player {}",
                    collision.tick, player.name, weapon_class, collision.projectile.launcher
                );
            }
        }
    }

    Ok(())
}
