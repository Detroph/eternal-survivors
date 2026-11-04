//! Anachronos at the cave mouth of the neutral camp: his talk window, a page of lore at a time
//!. The words are plans/hub-bronze-lore.md section 2's.

/// The greetings, one at random each time he is spoken to (A0a to A0c).
pub(super) const GREETINGS: [&str; 3] = [
    "Few mortals are brought to this cave mouth, and fewer understand why. Ask what you wish, and I will answer as I can.",
    "Back from the threads again. The sand on your boots is from another hour. Speak, if you have questions.",
    "Chromie tells me you are doing well. She is usually right about such things, if not always about when.",
];

/// A page of his: its words, the page Back leads to (none: the greeting) and the pages it leads
/// on to, each with its option's words.
pub(super) struct Page {
    pub id: &'static str,
    pub words: &'static str,
    pub back: Option<&'static str>,
    pub more: &'static [(&'static str, &'static str)],
}

/// The greeting's options, each to its page (A1 to A7; the A6 page adds the zones seen).
pub(super) const ASKS: [(&str, &str); 7] = [
    ("Who are you?", "who"),
    ("Why was I brought here?", "why"),
    ("What are the timeways?", "timeways"),
    ("Tell me of Nozdormu.", "nozdormu"),
    ("What is it you fear?", "fear"),
    ("What are the moments the portal shows?", "moments"),
    ("What of Chromie?", "chromie"),
];

/// The page Back from a zone's page leads to: the moments the portal shows.
pub(super) const MOMENTS: &str = "moments";

pub(super) const PAGES: &[Page] = &[
    Page {
        id: "who",
        words: "I am Anachronos, of the Bronze Dragonflight. When Nozdormu cannot be found, which is most of the time, I speak for him. You need not crane your neck; I hear well enough from up here, and I have not stepped on a mortal in centuries. I have watched this cave mouth since before your kingdoms had their names.",
        back: None,
        more: &[("You have been here so long?", "scarab_wall")],
    },
    Page {
        id: "scarab_wall",
        words: "Long before your grandfathers' grandfathers, the silithid and their qiraji masters poured out of {Area(3428)}. The flights fought them beside the night elves, and in the end we shut the city behind a wall of stone and scarabs. I was there when the scepter that held it was made, and there when it was broken in anger. In most threads the wall still holds. In a few it has fallen, and those I watch closely.",
        back: Some("who"),
        more: &[],
    },
    Page {
        id: "why",
        words: "Because you were somewhere your leaving would go unnoticed: a watch no one would remember, or the instant before a fall. We took you from there gently, and your thread closed behind you. Every hero at this fire came here the same way, whatever banner they once followed.",
        back: None,
        more: &[("What happens when I am overrun?", "overrun")],
    },
    Page {
        id: "overrun",
        words: "Then we pull you back before the moment closes over you, and you wake by the fire with sand in your boots. The strength you gathered in that moment stays there with it. What you learned comes home with you, and so does whatever you found.",
        back: Some("why"),
        more: &[("Why does another of my calling remember what I learned?", "by_calling")],
    },
    Page {
        id: "by_calling",
        words: "Because the sand sorts its lessons by calling. When one warrior learns something, every warrior at this fire is the wiser for it, and what one carries home the others may take up. I once tried to explain it more plainly to Chromie. She ended up drawing me a picture.",
        back: Some("overrun"),
        more: &[],
    },
    Page {
        id: "timeways",
        words: "Every moment that ever was, or will be, or nearly was, runs through these caverns like thread through a loom. My flight walks them and keeps them straight. Most of the time a thread mends itself. Sometimes it frays, and history begins to slip from its proper course. Those are the places we send you.",
        back: None,
        more: &[("Can I go inside the caverns?", "sealed")],
    },
    // no guardians stand at the mouth, so the plan's other middle sentence
    Page {
        id: "sealed",
        words: "No. The caverns are closed to mortal feet, and they will stay closed for a long while yet. The portal on the ridge will take you as far into the timeways as you need to go. It opens onto the moment we need held, and it keeps you from wandering where you could do harm.",
        back: Some("timeways"),
        more: &[],
    },
    Page {
        id: "nozdormu",
        words: "Nozdormu the Timeless, the Aspect of Time. The Titans set him to watch every hour there is, and he has never stopped watching. That leaves him very little room to be anywhere in particular. He comes here seldom. When he does, the sand in the great hourglass runs both ways at once, and even Chromie stops talking.",
        back: None,
        more: &[("Where is he now?", "where_he_is")],
    },
    Page {
        id: "where_he_is",
        words: "Everywhere he is needed, and a little behind on all of it. Do not take his absence for neglect. That you stand here at all is his doing. He saw the threads fraying long before I did, and told us to find hands to hold them.",
        back: Some("nozdormu"),
        more: &[],
    },
    Page {
        id: "fear",
        words: "Something is pulling at the threads. The Scourge, the black dragonflight and the old things under the sands of {Area(1377)} each do harm enough, but this is quieter work than theirs. A moment frays, and when we look closely, the knot has been picked loose on purpose. Whose hand did it, I cannot yet say, and that troubles me.",
        back: None,
        more: &[("Can it be stopped?", "steadied")],
    },
    Page {
        id: "steadied",
        words: "For now it can only be steadied. You step into a moment and hold it, and it settles back the way it ought to run, for a while. Then the knot is picked again and we send you back. Do not let that weigh on you. A thread held often enough may in time learn to hold itself.",
        back: Some("fear"),
        more: &[],
    },
    Page {
        id: MOMENTS,
        words: "Each place the portal shows is a moment fraying now. The nearest you know already: the fields of Elwynn, where the Defias rise and the kobolds pour out of the hills. Further down, the moments grow older and darker and harder to hold. The portal will show you each one once you have earned the sight of it. Ask me of any you have seen.",
        back: None,
        more: &[],
    },
    Page {
        id: "chromie",
        words: "Chromie. She chose that shape and that name herself, and she is fonder of both than she lets on. She is a good deal older than she looks, and I suspect she will matter more than either of us expects. Listen to her, even when she is muddled about what year it is.",
        back: None,
        more: &[],
    },
    // the zones' pages, by zone id: asked of from the moments' page, for the zones discovered
    Page {
        id: "elwynn",
        words: "The Defias grow bold in the fields by {Area(87)}, and the kobolds swarm out of the hills. In the true thread Stormwind dealt with them in its own time. In this one someone is hurrying them along. Hold the field until their leader shows himself. Edwin VanCleef does not like to be kept waiting.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "duskwood",
        words: "In {Area(10)} the dead do not stay down, and the worgen hunt in the dark. Something is calling them together, sooner than it should. Hold out until whatever calls them comes to see why they have stopped coming. Bring a light.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "wsg",
        words: "{Area(3277)}. In that thread no fire like ours was ever lit, and the Horde and the Alliance there have only ever met with weapons drawn. They will not know you. Do not hold back on their account. That moment made them what they are, and the moment is wrong.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "scarlet",
        words: "The Scarlet Crusade's cathedral. Zeal makes a poor keeper of time. It would drag the world back to the way it remembers it. Hold the courtyard until the doors open. The High Inquisitor will not thank you for coming.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "sunken_temple",
        words: "The sunken temple of the Atal'ai, under the {Area(8)}. The priests call to their Blood God, and the green dragons set to guard the temple dream something worse than sleep. I knew Eranikus once. Be quick with him, and kind if you can.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "feralas",
        words: "Eldre'Thalas, that the ogres call {Area(2557)}. The Highborne who stayed fed on something they should have let go long ago. Their ghosts still walk the courtyards, and the ogres sit on the ruins as if they built them. King Gordok thinks it is all his.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "arena",
        words: "The Gurubashi arena, in Stranglethorn. The Blood God's priests have taken the pit, and Hakkar presses at the edge of the world to come through. He will tear Stranglethorn apart in his hurry. Do not let him in.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "strat",
        words: "{Area(2017)}. You know what happened there, or you will. In this thread the Scourge holds the square before the Slaughterhouse, and the Baron is in no hurry to come out. Chromie has been busy in the Plaguelands of late. Ask her about it some time, but gently.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "brm",
        words: "{Area(25)}. Fire below, the black dragonflight above. Nefarian breeds his drakonids from the blood of every flight, mine among them. I take that personally. Hold out against the Dark Iron and the Blackrock until the son of Deathwing shows himself.",
        back: Some(MOMENTS),
        more: &[],
    },
    Page {
        id: "aq",
        words: "{Area(2737)}. I told you I was there. In this thread the wall is open, the swarm is loose across the sands, and what sleeps under the city is stirring. It is the worst of the moments the portal shows. When you go, go ready.",
        back: Some(MOMENTS),
        more: &[],
    },
];

/// His page `id`.
pub(super) fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_leads_to_a_page() {
        let leads = ASKS.iter().map(|(_, to)| *to).chain(PAGES.iter().flat_map(|p| p.more.iter().map(|(_, to)| *to).chain(p.back)));
        for to in leads {
            assert!(page(to).is_some(), "{to}");
        }
        for zone in ["elwynn", "duskwood", "wsg", "scarlet", "sunken_temple", "feralas", "arena", "strat", "brm", "aq"] {
            assert_eq!(page(zone).and_then(|p| p.back), Some(MOMENTS), "{zone}");
        }
    }
}
