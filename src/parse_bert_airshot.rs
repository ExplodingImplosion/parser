use std::f32::INFINITY;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use crate::demo::data::game_state::{Projectile, ProjectileType,PlayerState};
use crate::demo::data::DemoTick;
use crate::demo::gamevent::GameEvent;
use crate::demo::header::Header;
use crate::demo::parser::analyser::{UserInfo, CPID};
use crate::demo::parser::gamestateanalyser::{Building, Class, Dispenser, GameState, GameStateAnalyser, Kill, PlayerState as PlayerAliveState, Sentry, Team, Teleporter, UserId, World};
use crate::demo::vector::VectorXY;
use crate::{Demo, DemoParser,ParseError};
use crate::demo::data::game_state::Player;
use crate::demo::gameevent_gen::GameEventType::TeamPlayPointCaptured;
use crate::demo::message::packetentities::EntityId;

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

macro_rules! log_if_equal {
    ($msg:expr, $event_tick:expr, $tick:expr) => {
        if $event_tick == &$tick {
            println!("{} {}", $msg, $tick);
        }
    };
}

#[derive(Debug,Serialize,Deserialize)]
pub struct ParsedDemo {
    pub last_tick: DemoTick, // used while looping ?
    pub player_info: Vec<UserInfo>,
    pub header: Header,

    pub airshot_ticks: Vec<DemoTick>,
    pub kill_ticks: Vec<DemoTick>,
    pub death_ticks: Vec<DemoTick>,
}
const lmaocapacity: usize = 30;
impl ParsedDemo {
    pub fn new(header: Header) -> Self {
        ParsedDemo {
            last_tick: DemoTick::default(), // used while looping ?
            player_info: Vec::with_capacity(lmaocapacity),
            header,
            airshot_ticks: Vec::with_capacity(lmaocapacity),
            kill_ticks: Vec::with_capacity(60),
            death_ticks: Vec::with_capacity(lmaocapacity),
        }
    }

    pub fn finish(&mut self, state: &GameState) {
        // TODO add stuff
    }

    pub fn push_state(&mut self, mut game_state: &GameState) {
        if let Some(world) = game_state.world.as_ref() {
            // Other = no team on last
            // let mut team_on_last = Team::Other;
            // other = no team sac
            let mut is_sacing = Team::Other;
            for _tick in u32::from(self.last_tick)..u32::from(game_state.tick) {
                let tick = game_state.tick;

                // fill up the above stuff for this frame
                for (index, mut player) in game_state.players.iter().enumerate() {
                    if let (None, Some(info)) = (self.player_info.get(index), player.info.as_ref())
                    {
                        self.player_info.push(info.clone());
                    }
                }

                self.last_tick = tick;
            }
        }
    }
}
pub fn parse_demo(demo: Demo) -> Result<ParsedDemo,ParseError> {

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
    for player in state.players {
        println!("{}",player.flags);
    }
    Ok(parsed_demo)
}

pub fn get_name(player: &Player) -> String {
    player.info.as_ref().map_or("Unknown".to_string(),|info| info.name.clone())
}

pub fn get_user_id(player: &Player) -> UserId {
    player.info.as_ref().unwrap().user_id
}

pub fn find_on_team(players: Vec<&Player>, team: Team, max_capacity: usize) -> Vec<&Player> {
    let mut team_players: Vec<&Player> = Vec::with_capacity(max_capacity);
    for player in players {
        if player.team == team {
            team_players.push(player);
        }
    }
    team_players
}

pub fn is_alive(player: &Player) -> bool{
    player.state == PlayerAliveState::Alive
}