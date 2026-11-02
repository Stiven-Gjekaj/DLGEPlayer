//! Lists the chapters of each game for the chapter menu.
//!
//! The menu of the game is Lua bytecode, and the player does not run it.
//! These rules open the chapters:
//!
//! - a prologue is always open;
//! - the first chapter of a `Dark` route is always open;
//! - the first chapter of another route opens when the last chapter of the
//!   `Dark` route of the same character ends;
//! - another chapter opens when the chapter before it ends;
//! - a chapter that ended stays open.

use serde::Serialize;
use vm::Program;

use crate::profile::Profile;

/// One chapter of the menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chapter {
    /// The script name, for example `mb_aya_Dark_03`.
    pub name: String,
    /// 0 for Haunted Dark Bridal, 1 for MORE,BLOOD.
    pub game: i32,
    /// The short character name, for example `aya`. It is empty for a
    /// prologue.
    pub character: String,
    /// The route, for example `Dark`, or `prologue`.
    pub route: String,
    pub number: u32,
    pub finished: bool,
    pub open: bool,
}

/// Splits a script name such as `mb_aya_Dark_03` into the game prefix, the
/// character, the route, and the number.
pub fn split(name: &str) -> Option<(&str, &str, &str, u32)> {
    let mut parts = name.splitn(4, '_');
    let game = parts.next()?;
    let character = parts.next()?;
    let route = parts.next()?;
    let number = parts.next()?.parse().ok()?;
    Some((game, character, route, number))
}

/// Gives all chapters in the order of `script.bin`.
pub fn chapters(program: &Program, profile: &Profile, open_all: bool) -> Vec<Chapter> {
    let mut list: Vec<(&(i32, i32), &String)> = program.numbers.iter().collect();
    list.sort();
    let finished = |name: &str| profile.finished.contains(name);
    let route_names = |game: &str, character: &str, route: &str| -> Vec<&String> {
        list.iter()
            .map(|(_, n)| *n)
            .filter(|n| {
                split(n).is_some_and(|(g, c, r, _)| g == game && c == character && r == route)
            })
            .collect()
    };
    let mut out = Vec::new();
    for ((game, _), name) in &list {
        if let Some(rest) = name.split_once("_prologue_") {
            out.push(Chapter {
                name: name.to_string(),
                game: *game,
                character: String::new(),
                route: "prologue".into(),
                number: rest.1.parse().unwrap_or(0),
                finished: finished(name),
                open: true,
            });
            continue;
        }
        let Some((g, character, route, number)) = split(name) else {
            continue;
        };
        let names = route_names(g, character, route);
        let first = names.first().is_some_and(|n| n == name);
        let open = open_all
            || finished(name)
            || if first {
                route == "Dark"
                    || route_names(g, character, "Dark")
                        .last()
                        .is_some_and(|n| finished(n))
            } else {
                names
                    .iter()
                    .position(|n| n == name)
                    .and_then(|i| names.get(i.wrapping_sub(1)))
                    .is_some_and(|n| finished(n))
            };
        out.push(Chapter {
            name: name.to_string(),
            game: *game,
            character: character.to_string(),
            route: route.to_string(),
            number,
            finished: finished(name),
            open,
        });
    }
    out
}

/// Gives the chapter after a chapter in its route.
pub fn next_chapter(program: &Program, name: &str) -> Option<String> {
    let (game, character, route, number) = split(name)?;
    program
        .numbers
        .values()
        .filter(|n| {
            split(n).is_some_and(|(g, c, r, m)| {
                g == game && c == character && r == route && m == number + 1
            })
        })
        .min()
        .cloned()
}

/// Gives the chapter before a chapter in its route.
pub fn previous_chapter(program: &Program, name: &str) -> Option<String> {
    let (game, character, route, number) = split(name)?;
    let before = number.checked_sub(1)?;
    program
        .numbers
        .values()
        .filter(|n| {
            split(n).is_some_and(|(g, c, r, m)| {
                g == game && c == character && r == route && m == before
            })
        })
        .min()
        .cloned()
}
