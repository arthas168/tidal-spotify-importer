use anyhow::Error;
use rspotify::client::Spotify;
use rspotify::model::search::SearchResult;
use rspotify::model::track::FullTrack;
use rspotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::senum::{Country, IncludeExternal, SearchType};
use rspotify::util::get_token;

mod tidal;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let import_playlist = "0nAA9SMaBFq9HuthWj1E6D";

    // You can use any logger for debugging.
    pretty_env_logger::init();

    // The default credentials from the `.env` file will be used by default.
    let mut oauth = SpotifyOAuth::default()
        .scope("user-follow-read user-follow-modify")
        .build();

    match get_token(&mut oauth).await {
        Some(token_info) => {
            //TODO add structopt for file path
            let tidal = tidal::get_tidal_from_file(String::from("./tidal-tracks-deathcore.json")).await?; //TODO add buffered reading
            println!("Importing {} tracks", tidal.items.len());

            println!("Getting spotifty credentials");
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            let spotify = Spotify::default()
                .client_credentials_manager(client_credential)
                .build();


            let queries: Vec<(String, String)> = tidal.items.iter()
                .map(|track| {
                    let artist: String = track.item.artists.iter().map(|artist| artist.name.to_lowercase()).collect::<Vec<String>>().join(" ");
                    let query: String = vec![artist, track.item.title.to_lowercase()].join(" ");
                    (track.item.artist.name.to_lowercase(), query)
                }).collect();

            for (artist, query) in queries {
                if let Ok(find) = spotify.search(
                    query.as_str(),
                    SearchType::Track,
                    50,
                    0,
                    None,
                    None,
                ).await {
                    match find {
                        SearchResult::Tracks(tracks) => {
                            let tracks = tracks.items
                                .iter()
                                .filter(|track| {
                                    let artists = build_track_artists(track);
                                    artists.contains(&artist)
                                }).collect::<Vec<&FullTrack>>();
                            match tracks.first() {
                                None => println!("Could not find {} {}", artist, query),
                                Some(value) => log::debug!("Found {} {:?}", query, value.uri)
                            }

                        }
                        _ => {}
                    }
                };
            }
        }
        None => log::error!("Authentication failed"),
    };
    Ok(())
}

fn build_track_artists(track: &FullTrack) -> Vec<String> {
    track.artists.iter().map(|artist| artist.name.to_lowercase()).collect::<Vec<String>>()
}