use papilio_core::auth::create_token;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_lyric_offset_api_with_auth() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let base_url = "http://localhost:3000/api/v1";

    // 1. 构造真实的测试 Token
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "supersecret_change_me_in_production".to_string());
    let test_user_id = Uuid::new_v4();
    let token = create_token(test_user_id, "test_user".to_string(), &secret)?;

    let test_track_id = Uuid::new_v4();

    // 2. 验证 GET 接口 (带 Auth)
    let resp: reqwest::Response = client
        .get(&format!(
            "{}/tracks/{}/lyric-offset",
            base_url, test_track_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    println!("GET status: {}", resp.status());
    // 如果返回 404，说明 Auth 已通过，进入了处理逻辑（因为 ID 不存在）
    assert_eq!(
        resp.status().as_u16(),
        404,
        "Token should be valid, but track should not exist"
    );

    // 3. 验证 POST 接口 (带 Auth)
    let payload = json!({"offset_ms": 150});
    let resp_post: reqwest::Response = client
        .post(&format!(
            "{}/tracks/{}/lyric-offset",
            base_url, test_track_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await?;

    println!("POST status: {}", resp_post.status());
    assert!(
        resp_post.status().as_u16() != 401,
        "Should not be unauthorized with a valid token"
    );

    Ok(())
}
