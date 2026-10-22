//! The game's own messages with its server (esurv-protocol), over addon chat: the server's
//! lines become [`ServerEvent`]s for the screens, a [`Request`] goes to the server, and `hello`
//! goes once the hero is in the world, naming the screens this client has ([`ClientScreens`]); the
//! server stands in for the others (it takes the first offer of a choice, and leaves a victory as
//! it is).

use benilla_app::{NetHandlerApp, SendAddonMessage};
use benilla_protocol::{SessionEvent, SessionEventKind};
use bevy::prelude::*;
use esurv_protocol::{Reader, Screens, ToClient, ToServer, Wire, VERSION};

/// The screens this client has, as `hello` tells the server: each screen's plugin marks its own
/// (the server stands in for the rest).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientScreens(pub Screens);

/// A message from the server for the game's screens.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct ServerEvent(pub ToClient);

/// A request for the server, written by the game's screens.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct Request(pub ToServer);

/// The server's lines read so far: a long message comes in pieces.
#[derive(Resource, Default)]
struct ServerLines(Reader<ToClient>);

/// Adds the messages and their systems.
pub(crate) fn register(app: &mut App) {
    app.add_message::<ServerEvent>()
        .add_message::<Request>()
        .init_resource::<ServerLines>()
        .init_resource::<ClientScreens>()
        .net_handler(SessionEventKind::Chat, read_server_lines)
        .net_handler(SessionEventKind::Worldport, say_hello)
        .add_systems(Update, send_requests);
}

/// An addon line from the server: a message for the screens once it is whole.
fn read_server_lines(
    In(ev): In<SessionEvent>,
    mut lines: ResMut<ServerLines>,
    mut events: MessageWriter<ServerEvent>,
) {
    let SessionEvent::Chat(line) = ev else {
        return;
    };
    if !line.is_addon() {
        return;
    }
    if let Some(m) = lines.0.read(&line.text) {
        events.write(ServerEvent(m));
    }
}

/// The hero is in the world (the login's `SMSG_LOGIN_VERIFY_WORLD`, which needs no ack): the
/// client says it speaks the protocol, and which screens it has.
fn say_hello(
    In(ev): In<SessionEvent>,
    mut lines: ResMut<ServerLines>,
    mut requests: MessageWriter<Request>,
    screens: Res<ClientScreens>,
) {
    let SessionEvent::Worldport {
        needs_ack: false, ..
    } = ev
    else {
        return;
    };
    lines.0.clear();
    requests.write(Request(ToServer::Hello {
        version: VERSION,
        screens: screens.0,
    }));
}

/// Each request as addon lines to the server.
fn send_requests(mut requests: MessageReader<Request>, mut sends: MessageWriter<SendAddonMessage>) {
    for r in requests.read() {
        for line in r.0.lines() {
            sends.write(SendAddonMessage {
                prefix: line.prefix,
                message: line.message,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_says_this_client_has_no_screens_yet() {
        let lines = Request(ToServer::Hello {
            version: VERSION,
            screens: ClientScreens::default().0,
        })
        .0
        .lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].to_string(), format!("ESURVR\thello {VERSION}"));
    }

    #[test]
    fn server_lines_become_messages_and_the_numbers_line_does_not() {
        let mut lines = ServerLines::default();
        assert_eq!(lines.0.read("ESURV\thealth=230;damage=1.000"), None);
        assert_eq!(
            lines.0.read("ESURVT\tA%20swarm!"),
            Some(ToClient::Toast("A swarm!".into()))
        );
    }
}
