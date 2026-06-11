use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
}

pub fn generate_jwt(username: &str, role: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize + 24 * 3600; // Token expires in 24 hours
    
    let claims = Claims {
        sub: username.to_string(),
        role: role.to_string(),
        exp: expiration,
    };
    
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)?;
    Ok(token_data.claims)
}

pub fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    // 1. Check Authorization header (Bearer token)
    if let Some(auth_val) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }
    
    // 2. Check Cookie header for zeno_token
    if let Some(cookie_val) = headers.get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_val.to_str() {
            for pair in cookie_str.split(';') {
                let pair = pair.trim();
                if pair.starts_with("zeno_token=") {
                    return Some(pair["zeno_token=".len()..].to_string());
                }
            }
        }
    }
    
    None
}
