use rspotify::client::Spotify;
use rspotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::senum::{Country, SearchType, IncludeExternal};
use rspotify::util::get_token;

#[tokio::main]
async fn main() {
    let import_playlist = "0nAA9SMaBFq9HuthWj1E6D";

    // You can use any logger for debugging.
    pretty_env_logger::init();

    // The default credentials from the `.env` file will be used by default.
    let mut oauth = SpotifyOAuth::default()
        .scope("user-follow-read user-follow-modify")
        .build();

    println!(">>> Session one, obtaining refresh token and running some requests:");
    match get_token(&mut oauth).await {
        Some(token_info) => {
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            let spotify = Spotify::default()
                .client_credentials_manager(client_credential)
                .build();

            let playlists = spotify.current_user_playlists(10, 0).await;
            log::trace!("{:?}", playlists);

            let find = spotify.search(
                "slaughter to prevail demolisher",
                SearchType::Track,
                50,
                0,
                None,
                None
            ).await;

            println!("{:?}", find);

        }
        None => log::error!("Authentication failed"),
    };
}