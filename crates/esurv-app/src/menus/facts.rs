//! What the menus know of the save before a run, asked through the app's
//! [`crate::prerun::PreRunLink`] and kept until a change: each class's progress, the run options,
//! the codex and each hero's records and zone.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;
use esurv_protocol::{HeroFacts, PreRunAsk, Progress, RunOptions, ToClient};

use crate::prerun::PreRunLink;

/// The save's facts the menus show, as last told.
#[derive(Resource, Default)]
pub struct PreRunFacts {
    progress: HashMap<String, Progress>,
    options: Option<(RunOptions, f32)>,
    codex: Option<BTreeSet<String>>,
    heroes: HashMap<u64, HeroFacts>,
    /// Changes whenever what is known changes.
    version: u32,
}

impl PreRunFacts {
    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn progress(&self, class: &str) -> Option<&Progress> {
        self.progress.get(class)
    }

    /// The zones the save has discovered, by id (every class's progress carries the same list;
    /// none known: empty).
    pub fn zones_discovered(&self) -> &[String] {
        self.progress.values().next().map_or(&[], |p| p.zones_discovered.as_slice())
    }

    pub fn run_options(&self) -> Option<(RunOptions, f32)> {
        self.options
    }

    pub fn codex(&self) -> Option<&BTreeSet<String>> {
        self.codex.as_ref()
    }

    pub fn hero(&self, guid: u64) -> Option<&HeroFacts> {
        self.heroes.get(&guid)
    }

    /// Keeps what `told` says; the verbs refused.
    pub fn take(&mut self, told: Vec<ToClient>) -> Vec<String> {
        let mut refused = Vec::new();
        for m in told {
            match m {
                ToClient::Progress(p) => {
                    self.progress.insert(p.class.clone(), p);
                }
                ToClient::RunOptions { options, bonus } => self.options = Some((options, bonus)),
                ToClient::Codex { found } => self.codex = Some(found.into_iter().collect()),
                ToClient::Hero(h) => {
                    self.heroes.insert(h.hero, h);
                }
                ToClient::Refused(verb) => refused.push(verb),
                _ => continue,
            }
            self.version = self.version.wrapping_add(1);
        }
        refused
    }

    /// Asks `link` and keeps the answer; the verbs refused.
    pub fn ask(&mut self, link: &PreRunLink, ask: &PreRunAsk) -> Vec<String> {
        self.take(link.ask(ask))
    }

    /// Makes sure `class`'s progress, the run options and the codex are known.
    pub fn need_class(&mut self, link: Option<&PreRunLink>, class: &str) {
        if self.progress.contains_key(class) {
            return;
        }
        if let Some(link) = link {
            self.ask(
                link,
                &PreRunAsk::Facts {
                    class: class.into(),
                },
            );
        }
    }

    /// Makes sure hero `guid`'s records are known.
    pub fn need_hero(&mut self, link: Option<&PreRunLink>, guid: u64) {
        if self.heroes.contains_key(&guid) {
            return;
        }
        if let Some(link) = link {
            self.ask(link, &PreRunAsk::Hero(guid));
        }
    }

    /// Forgets the heroes' records (the roster changed: a run, a new or deleted hero).
    pub fn forget_heroes(&mut self) {
        if !self.heroes.is_empty() {
            self.heroes.clear();
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Forgets everything (back from a run, which earned and changed things).
    pub fn forget(&mut self) {
        *self = Self {
            version: self.version.wrapping_add(1),
            ..Self::default()
        };
    }
}
