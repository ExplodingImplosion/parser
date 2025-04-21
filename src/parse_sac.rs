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
    pub id: String,
    pub winner: Team,
    pub player_id_list: Vec<Vec<u8>>, // wtf??
    pub strats: Vec<Strat>,
    pub last_tick: DemoTick, // used while looping ?
    pub player_info: Vec<UserInfo>,
    pub header: Header,

    pub red: TeamTicks,
    pub blue: TeamTicks,
    pub last_tick_with_sac: (EntityId, DemoTick),
}

#[derive(Debug,Serialize,Deserialize)]
struct TeamTicks {
    on_last_ticks: Vec<DemoTick>,
    off_last_ticks: Vec<DemoTick>,
    sac_ticks: Vec<(EntityId,DemoTick)>,
    last_capped_ticks: Vec<DemoTick>,
    med_deaths_on_last: Vec<DemoTick>,
    sniper_ticks_per_last: Vec<u64>,
    spy_ticks_per_last: Vec<u64>,
    on_last_idx: usize,
}

const lmaocapacity: usize = 30;

impl TeamTicks {
    pub fn new() -> Self {
        assert_eq!(0usize.wrapping_sub(1).wrapping_add(1),0);
        TeamTicks {
            on_last_ticks: Vec::with_capacity(lmaocapacity),
            off_last_ticks: Vec::with_capacity(lmaocapacity),
            // 30 sacs seems reasonable, right?
            sac_ticks: Vec::with_capacity(lmaocapacity),
            last_capped_ticks: Vec::with_capacity(lmaocapacity),
            med_deaths_on_last: Vec::with_capacity(lmaocapacity),
            sniper_ticks_per_last: Vec::with_capacity(lmaocapacity),
            spy_ticks_per_last: Vec::with_capacity(lmaocapacity),
            on_last_idx: 0usize.wrapping_sub(1),
        }
    }

    pub fn push_sniper_spy_stuff(&mut self, tick: DemoTick) {
        self.sniper_ticks_per_last.push(0);
        self.spy_ticks_per_last.push(0);
        self.on_last_idx = self.on_last_idx.wrapping_add(1);
    }

    pub fn increase_sniper_spy_ticks(&mut self, team: &Vec<&Player>) {
        let i = self.on_last_idx;
        self.sniper_ticks_per_last[i] += if find_class(&team,Class::Sniper) { 1 } else { 0 };
        self.spy_ticks_per_last[i] += if find_class(&team,Class::Spy) { 1 } else { 0 };

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
            player_info: Vec::with_capacity(12),
            header,
            red: TeamTicks::new(),
            blue: TeamTicks::new(),
            last_tick_with_sac: Default::default(),
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

                let mut blue_team: Vec<&Player> = Vec::with_capacity(6);
                let mut red_team: Vec<&Player> = Vec::with_capacity(6);
                let mut soldiers: Vec<&Player> = Vec::with_capacity(4);
                let mut medics: Vec<&Player> = Vec::with_capacity(2);
                let mut demos: Vec<&Player> = Vec::with_capacity(2);

                // fill up the above stuff for this frame
                for (index, mut player) in game_state.players.iter().enumerate() {
                    if player.team == Team::Red {
                        red_team.push(player);
                    } else if player.team == Team::Blue {
                        blue_team.push(player);
                    }
                    if player.class == Class::Soldier {
                        soldiers.push(player);
                    } else if player.class == Class::Medic {
                        medics.push(player);
                    } else if player.class == Class::Demoman {
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

                let team_on_last = self.update_on_last_frames(game_state, tick);
                // The team on last should always be red, blue, or none.
                assert_ne!(team_on_last, Team::Spectator);


                if team_on_last == Team::Red {
                    // blue team might be sac'ing in
                    let blue_soldiers = find_soldiers(soldiers,Team::Blue);
                    let red_medics = find_medic(medics,Team::Red);
                    if is_alive(red_medics) {
                        find_sac_start(blue_soldiers, red_medics, &blue_team, red_team, tick,self);
                    }
                    self.blue.increase_sniper_spy_ticks(&blue_team);
                }
                else if team_on_last == Team::Blue {
                    // red team might be sac'ing in
                    let red_soldiers = find_soldiers(soldiers,Team::Red);
                    let blue_medics = find_medic(medics,Team::Blue);
                    if is_alive(blue_medics) {
                        find_sac_start(red_soldiers, blue_medics, &red_team, blue_team, tick,self);
                    }
                    self.red.increase_sniper_spy_ticks(&red_team);
                }

                // Calling this here means events are looped thru twice but fuck off!
                self.get_defending_med_died_on_last(team_on_last,game_state,tick);

                self.last_tick = tick;

            }
        }
    }

    fn get_defending_med_died_on_last(&mut self, team_on_last: Team, game_state: &GameState, tick: DemoTick) {
        // skip if not on last
        if team_on_last != Team::Red && team_on_last != Team::Blue {
            return
        }
        for (event_tick, event) in game_state.events.iter() {
            // skip events from earlier ticks
            if event_tick != &tick {
                continue;
            }
            match event {
                GameEvent::MedicDeath(event) => {
                    let mut medics: Vec<&Player> = Vec::with_capacity(1);
                    for (index, mut player) in game_state.players.iter().enumerate() {
                        if player.class == Class::Medic && player.team == team_on_last {
                            medics.push(player);
                        }
                    }
                    assert_eq!(medics.len(), 1);
                    let med = medics[0];
                    if event.user_id == get_user_id(med).into(){
                        // medic died
                        println!("med {} on last died {}", get_name(med), tick);
                        if team_on_last == Team::Red {
                            self.red.med_deaths_on_last.push(tick);
                        }
                        else if team_on_last == Team::Blue {
                            self.blue.med_deaths_on_last.push(tick);
                        } else {
                            println!("What the fuck?!");
                            assert!(false)
                        }
                    }
                }
                _ => {
                }
            }
        }
    }

    // Team::Other is treated as no change, Team::Spectator is treated as no longer on last
    fn update_on_last_frames(&mut self, game_state: &GameState, tick: DemoTick) -> Team {
        let mut team_on_last = Team::Other;
        for (event_tick, event) in game_state.events.iter() {
            match event {
                GameEvent::TeamPlayPointCaptured(event) => {
                    // println!("{:?}", event);

                    // if the cp is blue's second
                    if event.cp == CPID::BlueSecond as u8 {
                        // if red team capped blue second
                        if event.team == Team::Red as u8 {
                            // blue is on last now
                            if event_tick == &tick {
                                self.blue.on_last_ticks.push(tick);
                                self.red.push_sniper_spy_stuff(tick);
                                println!("blue on last {}", tick);
                            }
                            team_on_last = Team::Blue;
                        }
                        // otherwise, blue capped their second
                        else {
                            // blue is no longer on last
                            if event_tick == &tick {
                                self.blue.off_last_ticks.push(tick);
                                println!("blue off last {}", tick);
                            }
                            team_on_last = Team::Other;
                        }
                    }

                    // if the cp is red's second
                    else if event.cp == CPID::RedSecond as u8 {
                        // if blue team capped red second
                        if event.team == Team::Blue as u8 {
                            // red is on last now
                            if event_tick == &tick {
                                self.red.on_last_ticks.push(tick);
                                self.blue.push_sniper_spy_stuff(tick);
                                println!("red on last {}", tick);
                            }
                            team_on_last = Team::Red;
                        }
                        // otherwise, red team capped their second
                        else {
                            // red is no longer on last
                            if event_tick == &tick {
                                self.red.off_last_ticks.push(tick);
                                println!("red off last {}", tick);
                            }
                            team_on_last = Team::Other;
                        }
                    }
                    else if event.cp == CPID::RedLast as u8 {
                        if event_tick == &tick {
                            if event.team == Team::Blue as u8 {
                                println!("blue capped red's last {}", tick);
                                self.red.last_capped_ticks.push(tick);
                            } else {
                                println!("What the fuck?!");
                                assert!(false)
                            }
                        }
                        team_on_last = Team::Other;
                    }
                    else if event.cp == CPID::BlueLast as u8 {
                        if event_tick == &tick {
                            if event.team == Team::Red as u8 {
                                println!("red capped blue's last {}", tick);
                                self.blue.last_capped_ticks.push(tick);
                            } else {
                                println!("What the fuck?!");
                                assert!(false)
                            }
                        }
                        team_on_last = Team::Other; // someone capped last, no longer on last
                    }
                }
                _ => {}
            }
        }
        team_on_last
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
    let gaming = ticker.handler.get_parser_state();
    for class in &gaming.server_classes {
        println!("ID: {}, Name: {}",class.id,class.name);
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

pub fn find_class(team: &Vec<&Player>, class: Class) -> bool {
    let mut num_class = 0;
    for player in team{
        if Some(player).unwrap() == player{
            if player.class == class {
                num_class += 1;
                assert!(num_class < 3);
            }
        }
    }
    num_class > 0
}

// player bounding box sizes are 48-49 hu's deep and wide (there's inconsistent information online).
// 21 bounding boxes * 48.5 estimated bb size = 1018.5 hu's.
const SAC_DISTANCE: f32 = 1018.5;
pub fn find_sac_start(soldiers: Vec<&Player>, medic: &Player, soldier_team: &Vec<&Player>, other_team: Vec<&Player>, tick: DemoTick, demo_status: &mut ParsedDemo) -> DemoTick {
    for soldier in soldiers{
        if Some(soldier).unwrap() == soldier{
            if !is_alive(soldier) {
                continue;
            }
            // let soldier_pos = soldier_team.iter().position(&soldier);
            //
            // let mut everyone_else = soldier_team.clone();
            // everyone_else.remove(soldier_pos.unwrap());
            // assert!(!everyone_else.clone().contains(soldier));
            let everyone_else = get_without(&soldier_team,soldier);
            let other_team_alive = get_without(&other_team,soldier);
            let med_dist = get_dist(soldier,medic);
            let team_dist = get_min_dist(soldier,&everyone_else);
            let min_dist_player = get_min_dist_player(soldier,&everyone_else);
            // println!("--------------\nplayer: {} ({})\nmed dist: {}\nteam dist: {}\nteam: {} ({})\n--------------",
            //          get_name(soldier),soldier.state == PlayerState::Alive,med_dist,team_dist,get_name(min_dist_player),min_dist_player.class.to_string());
            //                                                      At least 5 players must be alive on both teams
            if med_dist < SAC_DISTANCE && med_dist < team_dist && everyone_else.len() > 3 && other_team_alive.len() > 4{
                let tick_info = (soldier.entity,tick);
                // Maybe make this even bigger to increase the threshold, but rn this bit here makes it
                // so that it's not just flooding output with sac ticks. this will eventually fuck up
                // when there are 2 soldiers close to each other because this happens in a loop.
                // The issue is that it's comparing both the tick nums and the entity ID, when it should
                // be prioritizing the tick num, and then invloving the entity ID, but wait why the fuck
                // is entity ID even included in this tuple? You can just check the tick num. Is there
                // even a point to last_tick_with_sac entity?!
                // FIXME change this!
                if tick - demo_status.last_tick_with_sac.1 < (66*8) {
                    demo_status.last_tick_with_sac = tick_info;
                    return DemoTick::from(0)
                }
                println!("--------\n{} {}\n{} {}\n--------",
                         demo_status.last_tick_with_sac.0,demo_status.last_tick_with_sac.1,soldier.entity,tick);
                // sac probably happening
                println!("{} is sacing {} on tick {}",get_name(soldier),get_name(medic),tick);
                println!("soldier team size: {} other team size: {}",everyone_else.len(),other_team_alive.len());
                demo_status.last_tick_with_sac = tick_info;
                if soldier.team == Team::Red {
                    demo_status.red.sac_ticks.push(tick_info);
                }
                else if soldier.team == Team::Blue {
                    demo_status.blue.sac_ticks.push(tick_info);
                }
                return tick;
            }
        }
    }
    DemoTick::from(0)
}

pub fn get_without<'a>(vec: &'a Vec<&'a Player>, player: &'a Player) -> Vec<&'a Player> {
    let mut without = vec.clone();
    // Exclude dead players
    without.retain(|this| this.state == PlayerAliveState::Alive);
    without.retain(|this| this != &player);
    // previous version, which somehow got fucked up after adding the dead player exclusion
    // without.remove(vec.iter().position(|this| this == &player).unwrap());
    assert!(!without.contains(&&player));
    without
}

pub fn get_name(player: &Player) -> String {
    player.info.as_ref().map_or("Unknown".to_string(),|info| info.name.clone())
}

pub fn get_user_id(player: &Player) -> UserId {
    player.info.as_ref().unwrap().user_id
}

pub fn get_min_dist(player: &Player, everyone_else: &Vec<&Player>) -> f32 {
    let mut min: f32 = INFINITY;
    for other_player in everyone_else {
        let dist = get_dist(player,other_player);
        if dist < min {
            min = dist;
        }
    }
    min
}

pub fn get_min_dist_player<'a>(player: &'a Player, everyone_else: &'a Vec<&'a Player>) -> Option<&'a Player> {
    let mut min: f32 = INFINITY;
    let mut idx: usize = 0;
    let mut i: usize = 0;
    for other_player in everyone_else {
        let dist = get_dist(player,other_player);
        if dist < min {
            min = dist;
            idx = i;
        }
        i += 1;
    }
    if everyone_else.is_empty() {
        return None
    }
    Some(everyone_else[idx])
}

pub fn get_min_dist_name(player: &Player, everyone_else: &Vec<&Player>) -> String {
    let mut min: f32 = INFINITY;
    let mut idx: usize = 0;
    let mut i: usize = 0;
    for other_player in everyone_else {
        let dist = get_dist(player,other_player);
        if dist < min {
            min = dist;
            idx = i;
        }
        i += 1;
    }
    get_name(everyone_else[idx])
}

pub fn get_dist(p1: &Player, p2: &Player) -> f32 {
    let positions = (p1.position, p2.position);
    let dist = (positions.0.x - positions.1.x, positions.0.y - positions.1.y, positions.0.z - positions.1.z);
    (dist.0 * dist.0 + dist.1 * dist.1 + dist.2 * dist.2).sqrt()
}

pub fn find_soldiers(soldiers: Vec<&Player>,team: Team) -> Vec<&Player> {
    find_on_team(soldiers, team,2)
}

pub fn find_medic(medics: Vec<&Player>,team: Team) -> &Player {
    find_on_team(medics, team, 1)[0]
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