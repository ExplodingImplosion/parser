use serde::{Deserialize, Serialize};
use crate::demo::data::game_state::{Projectile, ProjectileType};
use crate::demo::data::DemoTick;
use crate::demo::gamevent::GameEvent;
use crate::demo::header::Header;
use crate::demo::parser::analyser::{UserInfo, CPID};
use crate::demo::parser::gamestateanalyser::{Building, Class, Dispenser, GameState, GameStateAnalyser, Kill, PlayerState as PlayerAliveState, Sentry, Team, Teleporter, UserId, World};
use crate::demo::vector::VectorXY;
use crate::{Demo, DemoParser,ParseError};
use crate::demo::data::game_state::Player;
use js_sys::Function;
use crate::demo::gameevent_gen::GameEventType::TeamPlayPointCaptured;

#[derive(Debug,Serialize,Deserialize,PartialEq,Clone)]
enum StratType {
    SAC,
    SPY,
    SNIPE,
    DRY, // dry teamfight, no uber
    UBER,
    // maybe these will be included, maybe not
    DOUBLESAC,
    HEAVY,
    PYRO,

}

#[derive(Debug,Serialize,Deserialize,PartialEq,Clone)]
enum StratResult {
    MedDrop,
    MedForce,
    DemoKill,
    PlayerDeath,
    TeamWipe,
    PointTaken, // maybe get rid of this if only looking at last
    LastTaken,

    FriendlyMarker, // bits past this point are the same as above, but for friendly team
}

#[derive(Debug,Serialize,Deserialize,PartialEq,Clone)]
pub struct Strat {
    pub strat_type: StratType,
    pub tick: usize,
    pub result_tick: usize,
    pub team: Team,
    pub result: StratResult,
}

#[derive(Debug)]
pub struct ParsedDemo {
    pub id: String,
    pub winner: Team,
    pub player_id_list: Vec<Vec<u8>>, // wtf??
    pub strats: Vec<Strat>,
    pub last_tick: DemoTick, // used while looping ?
    pub player_info: Vec<UserInfo>,

}

pub struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vector3{
    pub fn new(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3{x, y, z}
    }
}
impl ParsedDemo {

    pub fn new(header: Header) -> Self {
        ParsedDemo {
            id: "default".to_string(),
            winner: Team::Spectator,
            player_id_list: Vec::new(),
            strats: Vec::new(),
            last_tick: DemoTick::default(), // used while looping ?
            player_info: Vec::new(),
        }
    }

    pub fn finish(&mut self, state: &GameState) {
        // TODO add stuff
    }

    pub fn push_state(&mut self, game_state: &GameState) {
        if let Some(world) = game_state.world.as_ref() {
            for _tick in u32::from(self.last_tick)..u32::from(game_state.tick) {

                let mut blue_team: Vec<&Player> = vec![];
                let mut red_team: Vec<&Player> = vec![];
                let mut soldiers: Vec<&Player> = vec![];
                let mut medics: Vec<&Player> = vec![];
                let mut demos: Vec<&Player> = vec![];
                for (index, mut player) in game_state.players.iter().enumerate() {
                    if player.team == Team::Red {
                        red_team.push(player);
                    }
                    else if player.team == Team::Blue {
                        blue_team.push(player);
                    }
                    if player.class == Class::Soldier{
                        soldiers.push(player);
                    }
                    else if player.class == Class::Medic{
                        medics.push(player);
                    }
                    else if player.class == Class::Demoman{
                        demos.push(player);
                    }


                    // if self.players.get(index).is_none() {
                    //     let mut new_player = Vec::with_capacity(
                    //         self.header.ticks as usize * PlayerState::PACKET_SIZE,
                    //     );
                    //     // backfill with defaults
                    //     new_player.resize(self.tick * PlayerState::PACKET_SIZE, 0);
                    //     self.players.push(new_player);
                    // };

                    if let (None, Some(info)) = (self.player_info.get(index), player.info.as_ref())
                    {
                        self.player_info.push(info.clone());
                    }
                }
                for (_,event) in game_state.events.iter() {
                    match event {
                        GameEvent::TeamPlayPointCaptured(event) => {
                            println!("{:?}", event);

                            // if the cp is blue's second
                            if (event.cp == CPID::BlueSecond as u8) {
                                // if red team capped blue second
                                if (event.team == Team::Red as u8) {
                                    // blue is on last now
                                    // game_state.blue_on_last_ticks.push(tick);
                                    println!("blue on last {}", game_state.tick);
                                }
                                // otherwise, blue capped their second
                                else {
                                    // blue is no longer on last
                                    // game_state.blue_off_last_ticks.push(tick);
                                    println!("blue off last {}", game_state.tick);
                                }
                            }

                            // if the cp is red's second
                            else if (event.cp == CPID::RedSecond as u8) {
                                // if blue team capped red second
                                if (event.team == Team::Blue as u8) {
                                    // red is on last now
                                    // game_state.red_on_last_ticks.push(tick);
                                    println!("red on last {}", game_state.tick);
                                }
                                // otherwise, red team capped their second
                                else {
                                    // red is no longer on last
                                    // game_state.red_on_last_ticks.push(tick);
                                    println!("red off last {}", game_state.tick);
                                }
                            }
                        }
                        _ => {

                        }
                    }
                }


            }
            self.last_tick = game_state.tick;
        }
    }
}

pub fn parse_demo(buffer: Box<[u8]>, progress: &Function) -> Result<(ParsedDemo),ParseError> {
    let demo = Demo::new(&buffer);

    let parser = DemoParser::new_with_analyser(demo.get_stream(), GameStateAnalyser::default());
    let (header, mut ticker) = parser.ticker()?;
    let total_ticks = header.ticks;
    let mut last_progress = 0.;

    let mut parsed_demo = ParsedDemo::new(header);

    while ticker.tick()? {
        parsed_demo.push_state(ticker.state());
        let new_progress =
            ((u32::from(ticker.state().tick) as f32 / total_ticks as f32) * 100.0).floor();
        if new_progress > last_progress {
            last_progress = new_progress;
            // let _ =  progress.call1(&JsValue::null(), &last_progress.into());
        }
    }

    parsed_demo.finish(ticker.state());
    let state = ticker.into_state();

    Ok(parsed_demo)
}
//
// pub fn lmao() {
//     if !is_last() {
//         return;
//     }
//
//     let team_atk_last = get_team_attacking_last();
//
//     let teams = ___; // teams of all player enttities
//
//     let soldiers = get_class_count();
//     let medics = get_class_count();
//
//     let spies = get_class_count();
//     let snipers = get_class_count();
// }
//
// pub fn get_class_count(class: Class, ) -> u8{
//
// }
//
// pub fn find_sac_start(soldiers,medics, tick: i64,) -> i64 {
//     for soldier in soldiers{
//         if soldier.get_distance_to(medics[]) < SAC_DISTANCE && !both_teams_close() {
//             // sac probably happening
//                 return tick;
//         }
//     }
//
// }

// pub fn parse_demo(buffer: Box<[u8]>, progress: &Function) -> Result<Results>