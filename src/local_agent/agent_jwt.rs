use std::fs;

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    // add other fields you need (email, roles, server_id, etc.)
}

pub fn create_jwt() -> anyhow::Result<String> {
    let priv_content = fs::read_to_string("private_key.pk8.pem")?;
    let private_pem = priv_content.as_bytes();
    let subject = "agent_1";
    let ttl_seconds = 3600;
    let exp = (Utc::now().timestamp() + ttl_seconds) as usize;

    let claims = Claims {
        sub: subject.to_owned(),
        exp,
    };

    // RS256 header by default when using from_rsa_pem
    let header = Header::new(Algorithm::RS256);

    let encoding_key = EncodingKey::from_rsa_pem(private_pem)?;
    let token = encode(&header, &claims, &encoding_key)?;
    Ok(token)
}

// Example usage
// let token = create_jwt(&priv_pem, "user123", 3600)?;
