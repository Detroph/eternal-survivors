//! The server side of WoW's SRP6 logon (the client side is `benilla-srp`'s): the server's public
//! key `B`, then checking the client's proof `M1` and answering with `M2`.
//!
//! WoW's variant: `N` and `g = 7` from `benilla-srp`, `k = 3`, all numbers little endian,
//! `u = SHA1(A | B)`, `S = (A · v^u)^b mod N`, `K` = the two SHA1 halves of `S` interleaved,
//! `M1 = SHA1((H(N) ^ H(g)) | H(I) | s | A | B | K)`, `M2 = SHA1(A | M1 | K)`.

use benilla_srp::{
    NormalizedString, GENERATOR, LARGE_SAFE_PRIME_LITTLE_ENDIAN, SESSION_KEY_LENGTH,
};
use num_bigint::{BigInt, Sign};
use rand::RngCore;
use sha1::{Digest, Sha1};

/// The SRP6 multiplier.
const K_VALUE: u32 = 3;

fn from_le(bytes: &[u8]) -> BigInt {
    BigInt::from_bytes_le(Sign::Plus, bytes)
}

fn to_32_le(v: &BigInt) -> [u8; 32] {
    let (_, bytes) = v.to_bytes_le();
    let mut out = [0u8; 32];
    out[..bytes.len().min(32)].copy_from_slice(&bytes[..bytes.len().min(32)]);
    out
}

fn sha1(parts: &[&[u8]]) -> [u8; 20] {
    let mut h = Sha1::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// The 40-byte session key from `S`: SHA1 of the even and of the odd bytes, interleaved.
fn interleave(s: &[u8; 32]) -> [u8; SESSION_KEY_LENGTH] {
    let even: Vec<u8> = s.iter().step_by(2).copied().collect();
    let odd: Vec<u8> = s.iter().skip(1).step_by(2).copied().collect();
    let (g, h) = (sha1(&[&even]), sha1(&[&odd]));
    let mut out = [0u8; SESSION_KEY_LENGTH];
    for i in 0..20 {
        out[i * 2] = g[i];
        out[i * 2 + 1] = h[i];
    }
    out
}

/// One account's logon in progress: the server has sent `B` and waits for the client's proof.
pub struct SrpServer {
    username: NormalizedString,
    salt: [u8; 32],
    verifier: BigInt,
    private_b: BigInt,
    public_b: [u8; 32],
}

impl SrpServer {
    /// Starts a logon for `username` whose password verifier is `verifier` under `salt`.
    pub fn new(username: NormalizedString, salt: [u8; 32], verifier: [u8; 32]) -> Self {
        let n = from_le(&LARGE_SAFE_PRIME_LITTLE_ENDIAN);
        let g = BigInt::from(GENERATOR);
        let v = from_le(&verifier);
        let mut rng = rand::thread_rng();
        loop {
            let mut b = [0u8; 19];
            rng.fill_bytes(&mut b);
            let private_b = from_le(&b);
            let public = (BigInt::from(K_VALUE) * &v + g.modpow(&private_b, &n)) % &n;
            let public_b = to_32_le(&public);
            // keep a B whose top byte is set: the client hashes it at full width either way
            if public_b[31] != 0 {
                return Self {
                    username,
                    salt,
                    verifier: v,
                    private_b,
                    public_b,
                };
            }
        }
    }

    pub fn salt(&self) -> &[u8; 32] {
        &self.salt
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_b
    }

    /// Checks the client's `A` and `M1`; on success returns the session key and the server's `M2`.
    pub fn verify(
        &self,
        client_public: &[u8; 32],
        client_proof: &[u8; 20],
    ) -> Option<([u8; SESSION_KEY_LENGTH], [u8; 20])> {
        let n = from_le(&LARGE_SAFE_PRIME_LITTLE_ENDIAN);
        let a = from_le(client_public);
        if (&a % &n) == BigInt::from(0) {
            return None;
        }
        let u = from_le(&sha1(&[client_public, &self.public_b]));
        let s = (&a * self.verifier.modpow(&u, &n)).modpow(&self.private_b, &n);
        let key = interleave(&to_32_le(&s));
        let n_hash = sha1(&[&LARGE_SAFE_PRIME_LITTLE_ENDIAN]);
        let g_hash = sha1(&[&[GENERATOR]]);
        let mut ng = [0u8; 20];
        for i in 0..20 {
            ng[i] = n_hash[i] ^ g_hash[i];
        }
        let user_hash = sha1(&[self.username.as_ref().as_bytes()]);
        let expected = sha1(&[
            &ng,
            &user_hash,
            &self.salt,
            client_public,
            &self.public_b,
            &key,
        ]);
        if &expected != client_proof {
            return None;
        }
        let server_proof = sha1(&[client_public, client_proof, &key]);
        Some((key, server_proof))
    }
}

#[cfg(test)]
mod tests {
    use benilla_srp::{generate_account, PublicKey, SrpClientChallenge};

    use super::*;

    #[test]
    fn a_benilla_client_and_this_server_agree_on_the_session_key() {
        let user = NormalizedString::new("HERO").unwrap();
        let pass = NormalizedString::new("SECRET").unwrap();
        let (salt, verifier) = generate_account(&user, &pass);
        let server = SrpServer::new(user.clone(), salt, verifier);
        let challenge = SrpClientChallenge::new(
            user,
            pass,
            GENERATOR,
            LARGE_SAFE_PRIME_LITTLE_ENDIAN,
            PublicKey::from_le_bytes(*server.public_key()).unwrap(),
            salt,
        );
        let (key, m2) = server
            .verify(challenge.client_public_key(), challenge.client_proof())
            .expect("the proof checks out");
        let client = challenge
            .verify_server_proof(m2)
            .expect("the client accepts the server's proof");
        assert_eq!(client.session_key(), &key);
    }

    #[test]
    fn a_wrong_password_is_refused() {
        let user = NormalizedString::new("HERO").unwrap();
        let (salt, verifier) = generate_account(&user, &NormalizedString::new("SECRET").unwrap());
        let server = SrpServer::new(user.clone(), salt, verifier);
        let challenge = SrpClientChallenge::new(
            user,
            NormalizedString::new("WRONG").unwrap(),
            GENERATOR,
            LARGE_SAFE_PRIME_LITTLE_ENDIAN,
            PublicKey::from_le_bytes(*server.public_key()).unwrap(),
            salt,
        );
        assert!(server
            .verify(challenge.client_public_key(), challenge.client_proof())
            .is_none());
    }
}
