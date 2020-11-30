use anyhow::Error;
use futures::{SinkExt, StreamExt};
use rspotify::client::Spotify;
use rspotify::model::cud_result::CUDResult;
use rspotify::model::search::SearchResult;
use rspotify::model::track::FullTrack;
use rspotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo};
use rspotify::senum::{Country, IncludeExternal, SearchType};
use rspotify::util::get_token;
use cli::Opts;
use crate::cli::get_opts_args;
use rspotify::model::user::PrivateUser;
use crate::tidal::TidalProvider;
use crate::provider::StreamingProvider;

mod tidal;
mod cli;
mod provider;

enum Platform {
    TIDAL,
    NONE
}

impl Platform {
    fn get_enum_from_string(value: String) -> Platform {
        let value = value.to_lowercase();
        let value = value.as_str();
        match value {
            "tidal" => Platform::TIDAL,
            _ => Platform::NONE
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let opts = get_opts_args();

    // TODO: hone down scope of app before deploying
    let mut oauth = SpotifyOAuth::default()
        .scope("user-read-recently-played playlist-modify-public playlist-modify-private user-follow-read user-follow-modify playlist-modify-private user-library-modify user-library-read")
        .build();

    match get_token(&mut oauth).await {
        Some(token_info) => {
            let platform = Platform::get_enum_from_string(opts.platform.clone());
            let (spotify, user) = get_spotify(token_info).await;
            match platform {
                Platform::TIDAL => {
                    let provider = TidalProvider::new(&opts);
                    provider.import(spotify, user).await?;
                }
                Platform::NONE => {}
            }
        }
        None => log::error!("Authentication failed, have you set up your .env file?"),
    };
    Ok(())
}

async fn get_spotify(token_info: TokenInfo) -> (Spotify, PrivateUser) {
    log::debug!("> Getting spotify credentials..");
    let client_credential = SpotifyClientCredentials::default()
        .token_info(token_info)
        .build();

    let spotify = Spotify::default()
        .client_credentials_manager(client_credential)
        .build();

    println!("> Getting user..");
    let user = spotify.current_user().await.expect("Failed to get user");

    (spotify, user)
}
