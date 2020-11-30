use rspotify::client::Spotify;
use rspotify::model::user::PrivateUser;
use anyhow::Error;
use async_trait::async_trait;

#[async_trait]
pub trait StreamingProvider {
    async fn import(&self, spotify: Spotify, user: PrivateUser) -> Result<(), Error>;
}