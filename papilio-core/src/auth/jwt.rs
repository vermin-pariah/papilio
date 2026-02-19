use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid, // User ID
    pub username: String,
    pub exp: usize, // Expiration time
    pub iat: usize, // Issued at
}

pub fn create_token(user_id: Uuid, username: String, secret: &str) -> Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .ok_or_else(|| anyhow::anyhow!("Timestamp overflow"))?
        .timestamp();

    let claims = Claims {
        sub: user_id,
        username,
        exp: expiration as usize,
        iat: Utc::now().timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?;

    Ok(token)
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::new(Algorithm::HS256),
    )?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_flow() {
        let user_id = Uuid::new_v4();
        let username = "test_user".to_string();
        let secret = "secret_key";
        
        let token = create_token(user_id, username.clone(), secret).unwrap();
        let claims = verify_token(&token, secret).unwrap();
        
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.username, username);
    }

    #[test]
    fn test_invalid_secret() {
        let user_id = Uuid::new_v4();
        let token = create_token(user_id, "user".into(), "right_secret").unwrap();
        
        let result = verify_token(&token, "wrong_secret");
        assert!(result.is_err());
    }
}
