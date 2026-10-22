//! Reading the client packets the world server handles.

use std::io;

use benilla_protocol::wire::{read_cstring, read_f32_le, read_u32_le, read_u64_le, read_u8};

/// `CMSG_AUTH_SESSION`: who the client says it is and its proof of the session key.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthSession {
    pub build: u32,
    pub account: String,
    pub client_seed: u32,
    pub proof: [u8; 20],
}

pub fn auth_session(mut r: &[u8]) -> io::Result<AuthSession> {
    let build = read_u32_le(&mut r)?;
    let _server_id = read_u32_le(&mut r)?;
    let account = read_cstring(&mut r)?;
    let client_seed = read_u32_le(&mut r)?;
    let mut proof = [0u8; 20];
    io::Read::read_exact(&mut r, &mut proof)?;
    // the addon block that follows is not needed
    Ok(AuthSession {
        build,
        account,
        client_seed,
        proof,
    })
}

/// `CMSG_CHAR_CREATE`: name, race, class, gender and the look.
#[derive(Debug, Clone, PartialEq)]
pub struct CharCreate {
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    /// skin, face, hair style, hair colour, facial hair.
    pub look: [u8; 5],
}

pub fn char_create(mut r: &[u8]) -> io::Result<CharCreate> {
    let name = read_cstring(&mut r)?;
    let mut b = [0u8; 8];
    io::Read::read_exact(&mut r, &mut b)?;
    Ok(CharCreate {
        name,
        race: b[0],
        class: b[1],
        gender: b[2],
        look: [b[3], b[4], b[5], b[6], b[7]],
    })
}

/// A body that is one full guid (delete, login, name query ...).
pub fn guid(mut r: &[u8]) -> io::Result<u64> {
    read_u64_le(&mut r)
}

/// `CMSG_CREATURE_QUERY`: the entry asked about.
pub fn creature_query(mut r: &[u8]) -> io::Result<u32> {
    read_u32_le(&mut r)
}

/// `CMSG_TUTORIAL_FLAG`: the tip seen, its 0-based id.
pub fn tip_seen(mut r: &[u8]) -> io::Result<u32> {
    read_u32_le(&mut r)
}

/// `CMSG_PING`: the sequence number to echo.
pub fn ping(mut r: &[u8]) -> io::Result<u32> {
    read_u32_le(&mut r)
}

/// Where a client's own hero is, from a movement packet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    pub flags: u32,
    pub position: [f32; 3],
    pub facing: f32,
}

/// The start of a `MSG_MOVE_*` body (the rest, transport and fall data, is not needed).
pub fn movement(mut r: &[u8]) -> io::Result<Moved> {
    let flags = read_u32_le(&mut r)?;
    let _time = read_u32_le(&mut r)?;
    let position = [
        read_f32_le(&mut r)?,
        read_f32_le(&mut r)?,
        read_f32_le(&mut r)?,
    ];
    let facing = read_f32_le(&mut r)?;
    Ok(Moved {
        flags,
        position,
        facing,
    })
}

/// `MSG_MOVE_TELEPORT_ACK` from the client: the guid and the teleport's counter.
pub fn teleport_ack(mut r: &[u8]) -> io::Result<(u64, u32)> {
    let guid = benilla_protocol::wire::read_packed_guid(&mut r)?;
    let counter = read_u32_le(&mut r)?;
    Ok((guid, counter))
}

/// A chat line the client sends (`CMSG_MESSAGECHAT`).
#[derive(Debug, Clone, PartialEq)]
pub struct ChatLine {
    pub chat_type: u32,
    pub language: u32,
    pub text: String,
}

impl ChatLine {
    /// Addon data (`SendAddonMessage`), not speech: the language is the only mark.
    pub fn is_addon(&self) -> bool {
        self.language == benilla_protocol::messages::LANGUAGE_ADDON
    }
}

/// `CMSG_MESSAGECHAT`: type, language, the whisper target or channel name for those two types,
/// then the text.
pub fn message_chat(mut r: &[u8]) -> io::Result<ChatLine> {
    use benilla_protocol::messages::{CHAT_TYPE_CHANNEL, CHAT_TYPE_WHISPER};
    let chat_type = read_u32_le(&mut r)?;
    let language = read_u32_le(&mut r)?;
    if matches!(chat_type, CHAT_TYPE_WHISPER | CHAT_TYPE_CHANNEL) {
        let _target = read_cstring(&mut r)?;
    }
    let text = read_cstring(&mut r)?;
    Ok(ChatLine {
        chat_type,
        language,
        text,
    })
}

/// The status byte of a one-byte body.
pub fn byte(mut r: &[u8]) -> io::Result<u8> {
    read_u8(&mut r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_create_request_from_the_client_reads_back() {
        let req = benilla_protocol::CharCreateReq {
            name: "Ana".into(),
            race: 4,
            class: 11,
            gender: 1,
            skin: 2,
            face: 3,
            hair_style: 4,
            hair_color: 5,
            facial_hair: 6,
        };
        let body = benilla_protocol::messages::char_create(&req);
        assert_eq!(
            char_create(&body).unwrap(),
            CharCreate {
                name: "Ana".into(),
                race: 4,
                class: 11,
                gender: 1,
                look: [2, 3, 4, 5, 6]
            }
        );
    }

    #[test]
    fn an_addon_line_from_the_client_reads_back() {
        use benilla_protocol::messages as m;
        let body = m::messagechat(m::CHAT_TYPE_PARTY, m::LANGUAGE_ADDON, "ESURVR\tchoose 1");
        let line = message_chat(&body).unwrap();
        assert!(line.is_addon());
        assert_eq!(line.text, "ESURVR\tchoose 1");
        let said = m::messagechat_whisper(7, "Ana", "hello there");
        let line = message_chat(&said).unwrap();
        assert!(!line.is_addon());
        assert_eq!(line.text, "hello there");
    }

    #[test]
    fn a_session_request_from_the_client_reads_back() {
        let body = benilla_protocol::messages::auth_session(
            5875,
            "ANYONE",
            77,
            &[9; 20],
            &benilla_protocol::messages::STOCK_SECURE_ADDONS,
        );
        let s = auth_session(&body).unwrap();
        assert_eq!(
            (s.build, s.account.as_str(), s.client_seed, s.proof),
            (5875, "ANYONE", 77, [9; 20])
        );
    }
}
