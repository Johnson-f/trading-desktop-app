//! PKCE (Proof Key for Code Exchange) primitives. Each sign-in flow
//! generates a fresh `(verifier, challenge)` pair: the verifier stays in
//! memory, the challenge goes in the authorize URL, and the verifier is
//! sent with the token exchange.
//!
//! Thin wrapper around `oauth2::PkceCodeChallenge` so the rest of the
//! module doesn't import oauth2 types directly.

use oauth2::{PkceCodeChallenge, PkceCodeVerifier};

pub struct PkcePair {
    pub verifier: PkceCodeVerifier,
    pub challenge: PkceCodeChallenge,
}

/// Generate a fresh, RFC-7636-compliant pair using SHA-256 transformation.
pub fn new_pair() -> PkcePair {
    // oauth2 5.x returns (challenge, verifier) — note the order.
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    PkcePair {
        verifier,
        challenge,
    }
}

/// Generate a CSRF state nonce for the authorize URL. Returned as the
/// URL-safe base64 of 32 random bytes.
pub fn random_state() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_pair_uses_s256() {
        let pair = new_pair();
        // method() returns &PkceCodeChallengeMethod which implements
        // Deref<Target=String>, so deref-coerce to &str for comparison.
        assert_eq!(pair.challenge.method().as_str(), "S256");
    }

    #[test]
    fn random_state_is_unique_per_call() {
        let a = random_state();
        let b = random_state();
        assert_ne!(a, b);
        assert!(a.len() >= 32);
    }
}
