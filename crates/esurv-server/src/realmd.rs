//! The logon server (realmd): SRP6 logon for any account name with the server's password, then
//! the realm list (the game offers one realm, [`crate::REALM_NAME`]). The session key of each
//! account that logged in is kept for the world server.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use benilla_srp::{
    generate_account, NormalizedString, GENERATOR, LARGE_SAFE_PRIME_LITTLE_ENDIAN,
    SESSION_KEY_LENGTH,
};

use crate::srp_server::SrpServer;

const CMD_AUTH_LOGON_CHALLENGE: u8 = 0x00;
const CMD_AUTH_LOGON_PROOF: u8 = 0x01;
const CMD_REALM_LIST: u8 = 0x10;
/// Logon results: success, and the code vmangos gives for a wrong password.
const RESULT_SUCCESS: u8 = 0x00;
const RESULT_UNKNOWN_ACCOUNT: u8 = 0x04;
/// The version challenge every mangos-family realmd sends (a constant, not a nonce).
const VERSION_CHALLENGE: [u8; 16] = [
    0xba, 0xa3, 0x1e, 0x99, 0xa0, 0x0b, 0x21, 0x57, 0xfc, 0x37, 0x3f, 0xb3, 0x69, 0xcd, 0xd2, 0xf1,
];
/// The realm list's closing bytes (vmangos writes `uint16(0x0002)` for 1.12).
const REALM_LIST_FOOTER: u16 = 0x0002;

/// The session keys of logged-in accounts, by uppercased account name, for the world server.
pub type SessionKeys = Arc<Mutex<HashMap<String, [u8; SESSION_KEY_LENGTH]>>>;

/// One line of the realm list.
#[derive(Debug, Clone, PartialEq)]
pub struct RealmEntry {
    pub name: String,
    /// `host:port` of its world server.
    pub address: String,
    /// How many heroes the account has (shown beside the name).
    pub characters: u8,
}

/// What the logon server needs.
#[derive(Clone)]
pub struct RealmdConfig {
    /// Every account logs in with this password (the launcher passes it to the client; in
    /// multiplayer it is the host's join password).
    pub password: String,
    /// The realm list, built anew for each request.
    pub realms: Arc<dyn Fn() -> Vec<RealmEntry> + Send + Sync>,
    pub keys: SessionKeys,
}

/// Accepts logons on `listener` until it closes, one thread per connection.
pub fn serve(listener: TcpListener, config: RealmdConfig) {
    for stream in listener.incoming().flatten() {
        let config = config.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &config);
        });
    }
}

fn read_exact<const N: usize>(r: &mut impl Read) -> Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}

fn handle(mut stream: TcpStream, config: &RealmdConfig) -> Result<()> {
    let mut logon: Option<(String, SrpServer)> = None;
    loop {
        let [cmd] = match read_exact::<1>(&mut stream) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        match cmd {
            CMD_AUTH_LOGON_CHALLENGE => {
                let [_protocol] = read_exact::<1>(&mut stream)?;
                let size = u16::from_le_bytes(read_exact::<2>(&mut stream)?) as usize;
                let mut body = vec![0u8; size];
                stream.read_exact(&mut body)?;
                // game(4) version(3) build(2) platform(4) os(4) locale(4) timezone(4) ip(4) then the name
                if body.len() < 30 {
                    bail!("logon challenge too short");
                }
                let len = body[29] as usize;
                let name = String::from_utf8_lossy(
                    body.get(30..30 + len).context("account name cut short")?,
                )
                .to_uppercase();
                let user = NormalizedString::new(&name).map_err(|e| anyhow::anyhow!("{e}"))?;
                let pass =
                    NormalizedString::new(&config.password).map_err(|e| anyhow::anyhow!("{e}"))?;
                let (salt, verifier) = generate_account(&user, &pass);
                let server = SrpServer::new(user, salt, verifier);
                let mut reply = vec![CMD_AUTH_LOGON_CHALLENGE, 0, RESULT_SUCCESS];
                reply.extend_from_slice(server.public_key());
                reply.push(1);
                reply.push(GENERATOR);
                reply.push(32);
                reply.extend_from_slice(&LARGE_SAFE_PRIME_LITTLE_ENDIAN);
                reply.extend_from_slice(server.salt());
                reply.extend_from_slice(&VERSION_CHALLENGE);
                reply.push(0); // security flag: none
                stream.write_all(&reply)?;
                logon = Some((name, server));
            }
            CMD_AUTH_LOGON_PROOF => {
                let a = read_exact::<32>(&mut stream)?;
                let m1 = read_exact::<20>(&mut stream)?;
                let _version_proof = read_exact::<20>(&mut stream)?;
                let [telemetry] = read_exact::<1>(&mut stream)?;
                for _ in 0..telemetry {
                    let _key = read_exact::<30>(&mut stream)?;
                }
                let [_security] = read_exact::<1>(&mut stream)?;
                let Some((name, server)) = logon.take() else {
                    bail!("proof before challenge")
                };
                match server.verify(&a, &m1) {
                    Some((key, m2)) => {
                        config.keys.lock().unwrap().insert(name, key);
                        let mut reply = vec![CMD_AUTH_LOGON_PROOF, RESULT_SUCCESS];
                        reply.extend_from_slice(&m2);
                        reply.extend_from_slice(&0u32.to_le_bytes()); // hardware survey id
                        stream.write_all(&reply)?;
                    }
                    None => {
                        stream.write_all(&[CMD_AUTH_LOGON_PROOF, RESULT_UNKNOWN_ACCOUNT])?;
                        return Ok(());
                    }
                }
            }
            CMD_REALM_LIST => {
                let _padding = read_exact::<4>(&mut stream)?;
                stream.write_all(&realm_list(&(config.realms)()))?;
            }
            other => bail!("unknown logon command {other:#x}"),
        }
    }
}

/// `CMD_REALM_LIST_Server` for `realms`.
fn realm_list(realms: &[RealmEntry]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&0u32.to_le_bytes()); // padding
    body.push(realms.len().min(255) as u8);
    for (i, r) in realms.iter().take(255).enumerate() {
        body.extend_from_slice(&0u32.to_le_bytes()); // realm type: normal
        body.push(0); // flags
        body.extend_from_slice(r.name.as_bytes());
        body.push(0);
        body.extend_from_slice(r.address.as_bytes());
        body.push(0);
        body.extend_from_slice(&0.0f32.to_le_bytes()); // population
        body.push(r.characters);
        body.push(1); // category (time zone)
        body.push(i as u8); // id
    }
    body.extend_from_slice(&REALM_LIST_FOOTER.to_le_bytes());
    let mut out = vec![CMD_REALM_LIST];
    out.extend_from_slice(&(body.len() as u16).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(password: &str) -> (String, SessionKeys) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let keys: SessionKeys = Arc::default();
        let config = RealmdConfig {
            password: password.into(),
            realms: Arc::new(|| {
                vec![
                    RealmEntry {
                        name: "Area 12".into(),
                        address: "127.0.0.1:9001".into(),
                        characters: 2,
                    },
                    RealmEntry {
                        name: "Area 10".into(),
                        address: "127.0.0.1:9002".into(),
                        characters: 0,
                    },
                ]
            }),
            keys: keys.clone(),
        };
        std::thread::spawn(move || serve(listener, config));
        (format!("127.0.0.1:{port}"), keys)
    }

    #[test]
    fn the_benilla_client_logs_in_and_sees_the_zones() {
        let (host, keys) = start("LOCAL");
        let logon = benilla_protocol::logon(&host, "anyone", "local").expect("logon succeeds");
        let names: Vec<&str> = logon.realms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Area 12", "Area 10"]);
        assert_eq!(logon.realms[0].address, "127.0.0.1:9001");
        assert_eq!(logon.realms[0].characters, 2);
        assert_eq!(keys.lock().unwrap().get("ANYONE"), Some(&logon.session_key));
    }

    #[test]
    fn a_wrong_password_is_turned_away() {
        let (host, _) = start("LOCAL");
        assert!(benilla_protocol::logon(&host, "anyone", "nope").is_err());
    }
}
