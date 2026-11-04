//! The guides' walks and words: in the neutral camp at the Caverns of Time, Wren's for the
//! Alliance and Rezza's for the Horde, from Chromie and back to her
//!.
//! The neutral camp's words are the advisor's, in the Bronze Dragonflight's theme.

use super::{Did, Ends, Part, Stop, Toward, Walk, Words, FIRE_WAIT};
use crate::game::camp::Keeper;
use esurv_content::TimePeriod;

/// Chromie's first words to the hero, by its race and the time period it was pulled in from (its
/// `time_period`): she knows when and where she fetched it from. Written
/// by two writers, one for each side, and checked by two reviewers for lore and for lines that sound
/// written by a machine.
pub(super) const CHROMIE_FIRST: &[(&str, TimePeriod, &str)] = &[
    ("human", TimePeriod::Past, "Has Prince Arthas named that new foal of his yet? I'd bet on something grand."),
    ("human", TimePeriod::Present, "Is Lady Prestor still whispering in the boy king's ear? Something about that woman makes my scales itch."),
    ("human", TimePeriod::Future, "My, you're from King Anduin's Stormwind! He was such a small boy the last time I looked in."),
    ("dwarf", TimePeriod::Past, "Muradin's not back from Lordaeron yet, is he? Magni keeps setting a place for him at supper."),
    ("dwarf", TimePeriod::Present, "Any letter from Moira yet? Her father's paced a groove into the throne room floor."),
    ("dwarf", TimePeriod::Future, "Your old king's still standing in the throne room, all sparkly? I tap him for luck on my way past."),
    ("gnome", TimePeriod::Past, "Oh, from {Area(133)} with all its lights still on! I got lost down there once, and High Tinker Mekkatorque himself had to come and fetch me."),
    ("gnome", TimePeriod::Present, "You've come straight out of {Area(133)}, haven't you? There's a little green glow about your ears, so I'll stand just here, if that's all right."),
    ("gnome", TimePeriod::Future, "Ooh, you're from after Operation: {Area(133)}! I take my pocket watch to New Tinkertown to be mended."),
    ("night_elf", TimePeriod::Past, "Moss from the barrow dens on your boots! I had to tiptoe past every last sleeping druid to fetch you."),
    ("night_elf", TimePeriod::Present, "Fandral's still trimming the branches up in {Area(1657)}? He gets so cross when I land on them."),
    ("night_elf", TimePeriod::Future, "Is that smoke from {Area(141)} still in your hair? Oh, you poor dear. Come and sit, and we won't talk about trees today."),
    ("orc", TimePeriod::Past, "Welcome to Azeroth! You haven't heard of it yet, have you? Well, your clan will be talking of little else soon."),
    ("orc", TimePeriod::Present, "Still a scorpid or two in your pack from {Area(14)}? Do check before you sit down, there's a dear."),
    ("orc", TimePeriod::Future, "Lok'tar, and do tell Warchief Thrall... no, Garrosh? Vol'jin? Oh, I've lost count of your Warchiefs."),
    ("troll", TimePeriod::Past, "Is Sen'jin keeping well? Do tell him... oh, no, best not tell him anything at all."),
    ("troll", TimePeriod::Present, "{Area(367)} is lovely this time of year, isn't it? Shame about the neighbours over on the {Area(368)}."),
    ("troll", TimePeriod::Future, "Did you sail over from the {Area(368)}? They look ever so tidy now Zalazane's gone."),
    ("tauren", TimePeriod::Past, "Are the centaur still chasing your tribes across the Barrens? You can rest here a while, they won't find you."),
    ("tauren", TimePeriod::Present, "Greetings, friend from {Area(1638)}! I always take the lift up, those bluffs are far too tall for my little gnome legs."),
    ("tauren", TimePeriod::Future, "Do give my best to High Chieftain Baine! Oh, bother, is it still Cairne in your day?"),
    ("undead", TimePeriod::Past, "You're looking a little pale for a farmer from Tirisfal. Too long out in the fields?"),
    ("undead", TimePeriod::Present, "Varimathras still lurking about the Dark Lady's throne? Do keep an eye on him for me."),
    ("undead", TimePeriod::Future, "Off to the {Area(1497)}? Oh, no, there's nothing much there for you any more, is there?"),
];

/// Her first words to a hero of a race the table lacks, or one she knows nothing of.
pub(super) const CHROMIE_FIRST_ANY: &str = "There you are! Or there you will have been, I lose track. Welcome to the {Area(1941)}!";

/// Chromie's first words to a hero of race `race` pulled in from time period `time_period`.
pub(super) fn chromie_first(race: &str, time_period: TimePeriod) -> &'static str {
    CHROMIE_FIRST.iter().find(|(r, p, _)| *r == race && *p == time_period).map_or(CHROMIE_FIRST_ANY, |(_, _, words)| words)
}

/// Chromie's own words after her first: her welcome to the warband's first hero, and how she
/// recruits more, said as bubbles before she calls the guide over.
pub(super) const CHROMIE_WELCOME: &[&str] = &[
    "I'm Chromie, of the Bronze Dragonflight. Don't mind the gnome. I picked this shape myself, and I'm very fond of it.",
    "We plucked you out of your own timeline, from a moment where nobody would miss you for a while. Time is fraying, and the flight needs hands to hold it.",
    "Don't worry about home. Your own thread closed up neatly behind you, I saw to it myself.",
    "We'll need more than you, mind. Whenever you want another hero at this fire, come and talk to me.",
    "I'll go and find someone in another moment who won't be missed for a bit, and bring them through. Alliance or Horde, whatever race and calling you like.",
    "See the standards behind me? Alliance and Horde, side by side. Everyone shares the one fire here, so mind your manners!",
];

/// Her talk window, outside the lesson's end.
pub(super) const CHROMIE_HELLO: &str = "Hello again! Need another pair of hands at the fire? I know plenty of moments where nobody would miss one for a while.";
/// Her talk window at the lesson's end, the hero brought back to her.
pub(super) const CHROMIE_SEND: &str = "Ready? Then through the Bronze Portal you go. I'll be watching, and I'll have you out before the moment closes. Bring back whatever you can!";
pub(super) const SEND_ME: &str = "Send me through the portal. <Begin your first run>";
/// Her options: a new hero first, her words on this and that, then the lesson again.
pub(super) const RECRUIT_ONE: &str = "I'd like to recruit a new hero. <Create a new hero>";
pub(super) const TELL_AGAIN: &str = "Can you explain everything again? <Tour around camp>";

/// Her words on this and that: each question, to its page.
pub(super) const CHROMIE_ASKS: &[(&str, &str)] = &[
    ("Why do you look like a gnome?", "gnome"),
    ("What is the Bronze Portal?", "portal"),
    ("Where do the other heroes come from?", "heroes"),
];

pub(super) const CHROMIE_PAGES: &[(&str, &str)] = &[
    ("gnome", "Because I like it! Big wings are no use in a tent, and gnomes are so practical. Anachronos thinks I should be more dignified. He would, he's the size of a house."),
    ("portal", "It opens onto the moments that are fraying, one at a time. Step through and hold the moment as long as you can, and we'll pull you back before it closes over you. Anything you learn in there, and anything you find, you bring home. Everything else stays in the moment."),
    ("heroes", "From moments where nobody will miss them for a while. A sentry dozing at his post, say, or a fisherman out past dark. Their threads close up behind them neatly enough. Ask me whenever you want another one and I'll go looking."),
];

/// The portal's words at the lesson's end, Chromie's after the guide's hand-back, the more heroes
/// last.
const CHROMIE_PORTAL: &[&str] = &[
    "Thank you! Now, the important part. The Bronze Portal beside me opens onto the moments that are fraying.",
    "Step through, hold the moment as long as you can, and we pull you back before it closes over you.",
    "Your lessons and any gear you find come back here with you. Talk to me when you're ready and I'll send you through.",
    CHROMIE_MORE_HEROES,
];

/// Her last words before she sends the hero on its first run: a hero of every class for each side
/// makes the camp livelier.
pub(super) const CHROMIE_MORE_HEROES: &str = "Before I send you off, could you bring a few more friends around? A hero of each class for the Alliance and the Horde both, and the camp gets ever so much livelier.";

/// The guide's words in the neutral camp's walks that both sides share in shape.
const SEND_TO_CHROMIE: &str = "Speak with Chromie to begin your first run.";

/// Wren's walk at the Caverns of Time.
pub(super) const WREN_AT_THE_CAVERNS: Walk = Walk {
    words: Words {
        first: "So Chromie's handed you over. I'm Wren Ashcombe. I ran parcels through the Trade District in Stormwind before the Alliance sent me south to this camp, and now I run errands for our half of it, down in the north basin. Shall I walk you round?",
        show_me: "Show me around the camp. <Tour around camp>",
        own_way: "I know my way around.",
        skipped: "Then I'll leave you to it. If you lose your bearings among the tents, come back up to the fire and look for me. I'm usually here between errands.",
        next: "Where to next?",
        again: "What should I do here?",
        enough_option: "That's enough for now, thank you.",
        enough: "As you like. If you want to finish the walk later, come and find me by the fire. I'll be there between errands.",
        wandered_off: "Hold on, you've wandered away from the tents! The scorpids out that way bite, so come back and we'll carry on from where we stopped.",
        seen_off: "Light keep you out there. When you come back the fire will still be burning, and I'll be beside it with the camp's news.",
        written_in: "You've walked the whole camp with me now, so I'll see that Marshal Hale writes your name into his roll as a Recruit of this camp.",
        after: "Come and stand in the shade a moment, the sand out here is hot enough to bake bread on. I keep the camp's Guide in my satchel, written out in Tobbin's neat hand, and you're welcome to read it whenever something in the camp or out there has you puzzled.",
        read_guide: "Let me read the Guide. <Open Guide>",
        walk_again: "Walk me round the camp again. <Tour around camp>",
        farewell: "Farewell, Wren.",
    },
    stops: &[
        Stop {
            via: &[],
            at: None,
            faces: Toward::Chromie,
            lines: &[(Keeper::Wren, "Coming, coming! I'm Wren Ashcombe, I run errands for the Alliance's half of this camp. Follow me, I'll show you round.")],
            chromie: &["Off you go, then. Wren knows every tent in the north basin. I keep losing mine."],
            waves: None,
            try_it: "Follow Wren.",
            ends: Ends::After(FIRE_WAIT),
            part: Part::Meet,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Between(Keeper::Chronicler, Keeper::Merchant),
            lines: &[
                (Keeper::Wren, "That's Tobbin Quillsprocket at his library table. He chronicles every deed of our camp's heroes, and yours will go in his books too."),
                (Keeper::Wren, "You earn Moments for your milestones and for every moment you hold steady. You spend them with Maribel Thorne, here at the Market."),
                (Keeper::Merchant, "Hi Wren, did you just bring in a new customer?"),
                (Keeper::Wren, "Maribel prices her goods in Moments, and she sells all sorts, from companions to tent decorations."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Tobbin Quillsprocket or Maribel Thorne.",
            ends: Ends::By(&[Did::Talked(Keeper::Chronicler), Did::Talked(Keeper::Merchant), Did::Opened("achievements"), Did::Opened("shop")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Person(Keeper::OrbKeeper),
            lines: &[
                (Keeper::Wren, "Zazzi Glimerspell keeps the Transmutation Orb. She learned her art in {Area(279)}, before they shut the city behind its barrier."),
                (Keeper::Wren, "With the Orb she tugs at the strands of time and can change your looks or your race. She can even change the banner you serve."),
                (Keeper::OrbKeeper, "Ooh, someone new for the Orb! Hold still, the ears are the tricky part."),
                (Keeper::Wren, "She's a bit quirky, but you'll get used to her."),
            ],
            chromie: &[],
            waves: Some(Keeper::OrbKeeper),
            try_it: "Use the Transmutation Orb.",
            ends: Ends::By(&[Did::Talked(Keeper::OrbKeeper), Did::Opened("appearance")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Person(Keeper::Trainer),
            lines: &[
                (Keeper::Wren, "Thurgen Ironbrow hauled that forge wagon all the way from {Area(1537)}. As if Tanaris weren't hot enough."),
                (Keeper::Wren, "He trains every calling. Speak to him when you want to choose your talents."),
                (Keeper::Wren, "Your own gear hangs on the Weapon Rack and the armour stand by his forge. Whatever you learn or find out there, you bring home with you."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Thurgen Ironbrow.",
            ends: Ends::By(&[Did::Talked(Keeper::Trainer), Did::Opened("talents"), Did::Opened("equipment")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Between(Keeper::Marshal, Keeper::Planner),
            lines: &[
                (Keeper::Wren, "And this is your superior, Marshal Corwin Hale. He answers for the Alliance's half of the camp."),
                (Keeper::Marshal, "King's Honor, Recruit."),
                (Keeper::Wren, "You choose your moment at the Bronze Portal beside Chromie, and she sends you through. The Marshal expects your report when you come back."),
                (Keeper::Wren, "Lorendir Nightbough, the night elf with the mallet, can show you where and how to decorate the camp."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Marshal Corwin Hale.",
            ends: Ends::By(&[Did::Talked(Keeper::Marshal), Did::Talked(Keeper::Planner)]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Chromie,
            lines: &[(Keeper::Wren, "Here we are, back where you started. Chromie, they're all yours.")],
            chromie: CHROMIE_PORTAL,
            waves: None,
            try_it: SEND_TO_CHROMIE,
            ends: Ends::By(&[Did::Chromie]),
            part: Part::Back,
        },
    ],
};

/// Rezza's walk at the Caverns of Time.
pub(super) const REZZA_AT_THE_CAVERNS: Walk = Walk {
    words: Words {
        first: "Chromie's done with you? Good. Dragons talk too much. I'm Rezza. I ran messages between {Area(362)} and the gates of Orgrimmar since I could walk, and now I run them for the Horde's half of this camp, down in the south basin. Want me to show you round?",
        show_me: "Show me the camp. <Tour around camp>",
        own_way: "I'll find my own way.",
        skipped: "Suit yourself. If you get lost down there, come back up to the fire. I'm here when I'm not running.",
        next: "Where to next?",
        again: "What do I do here?",
        enough_option: "That's enough for now.",
        enough: "Fine. Come find me at the fire when you want the rest.",
        wandered_off: "Hey! Wrong way. You want a scorpid sting? Back to the tents, and we'll pick up where we stopped.",
        seen_off: "Strength and honor. The fire will be burning when you get back, and I'll have the camp's news.",
        written_in: "That's the whole camp. I'll tell the Warlord to put your name on his roll, as a Recruit of the Horde.",
        after: "Back again? Get in the shade, the sand's hot enough to cook on. I carry the camp's Guide, copied out by Aldous in that spidery hand of his. Read it when something out there makes no sense.",
        read_guide: "Let me read the Guide. <Open Guide>",
        walk_again: "Walk me round again. <Tour around camp>",
        farewell: "Farewell, Rezza.",
    },
    stops: &[
        Stop {
            via: &[],
            at: None,
            faces: Toward::Chromie,
            lines: &[(Keeper::Wren, "I'm here, I'm here. I'm Rezza, I run for the Horde's half of this camp. Keep up, and I'll show you round.")],
            chromie: &["Be nice! Rezza knows the south basin inside out. Off you go!"],
            waves: None,
            try_it: "Follow Rezza.",
            ends: Ends::After(FIRE_WAIT),
            part: Part::Meet,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Between(Keeper::Chronicler, Keeper::Merchant),
            lines: &[
                (Keeper::Wren, "That's Aldous Greymarch. He wrote for the Scarlet Crusade once, before he died and thought better of it. Now he chronicles every deed of the Horde's heroes."),
                (Keeper::Wren, "Milestones and moments you hold steady get paid in Moments. Jun'zabi there prices everything in them."),
                (Keeper::Merchant, "Rezza! Ya bring me somebody wit' Moments to spend, mon?"),
                (Keeper::Wren, "Not yet. Give them a run first."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Aldous Greymarch or Jun'zabi.",
            ends: Ends::By(&[Did::Talked(Keeper::Chronicler), Did::Talked(Keeper::Merchant), Did::Opened("achievements"), Did::Opened("shop")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Person(Keeper::OrbKeeper),
            lines: &[
                (Keeper::Wren, "Zuli'ka keeps the Voodoo Mask, in that hut. A witch doctor's art."),
                (Keeper::Wren, "Through the Mask she twists the strands of time. Ask her and she'll change your face or your race. She'll even put you under the other banner."),
                (Keeper::OrbKeeper, "Pick a new skin, mon. Da loa don't mind which one ya wear."),
                (Keeper::Wren, "She talks to things nobody else can see, and I don't ask."),
            ],
            chromie: &[],
            waves: Some(Keeper::OrbKeeper),
            try_it: "Use the Voodoo Mask.",
            ends: Ends::By(&[Did::Talked(Keeper::OrbKeeper), Did::Opened("appearance")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Person(Keeper::Trainer),
            lines: &[
                (Keeper::Wren, "Dargash Stonefist. He drilled grunts in the {Area(363)}, and he hasn't stopped shouting since."),
                (Keeper::Wren, "He trains every calling, casters too. Speak to him when you want your talents."),
                (Keeper::Wren, "Your gear hangs on the tauren rack by the forge. Anything you learn or find out there, you bring back with you."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Dargash Stonefist.",
            ends: Ends::By(&[Did::Talked(Keeper::Trainer), Did::Opened("talents"), Did::Opened("equipment")]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Between(Keeper::Marshal, Keeper::Planner),
            lines: &[
                (Keeper::Wren, "This is Warlord Garnok Ashmaw. He answers for the Horde's half of the camp."),
                (Keeper::Marshal, "Lok'tar, Recruit."),
                (Keeper::Wren, "You pick your moment at the Bronze Portal by Chromie's table, and she sends you through. The Warlord is who you report to when you come back."),
                (Keeper::Wren, "Tohan Highmesa, the tauren with the mallet, can show you where and how to decorate the camp."),
            ],
            chromie: &[],
            waves: None,
            try_it: "Speak with Warlord Garnok Ashmaw.",
            ends: Ends::By(&[Did::Talked(Keeper::Marshal), Did::Talked(Keeper::Planner)]),
            part: Part::Walk,
        },
        Stop {
            via: &[],
            at: None,
            faces: Toward::Chromie,
            lines: &[(Keeper::Wren, "Back to you, Chromie. They'll do.")],
            chromie: CHROMIE_PORTAL,
            waves: None,
            try_it: SEND_TO_CHROMIE,
            ends: Ends::By(&[Did::Chromie]),
            part: Part::Back,
        },
    ],
};

/// Chromie calls the guide over at the end of her welcome (the guide's own name in it).
pub(super) const CALLS_WREN: &str = "Wren! Over here! I've got a new one for you!";
pub(super) const CALLS_REZZA: &str = "Rezza! Rezza! Another one for you!";
