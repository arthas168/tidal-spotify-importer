use anyhow::Error;
use futures::{StreamExt, SinkExt};
use rspotify::client::Spotify;
use rspotify::model::cud_result::CUDResult;
use rspotify::model::search::SearchResult;
use rspotify::model::track::FullTrack;
use rspotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::senum::{Country, IncludeExternal, SearchType};
use rspotify::util::get_token;

mod tidal;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let test_import_playlist = "66cX8bDIrHpiXY5J07Orzs";

    // You can use any logger for debugging.
    pretty_env_logger::init();

    // The default credentials from the `.env` file will be used by default.
    let mut oauth = SpotifyOAuth::default()
        .scope("user-read-recently-played playlist-modify-public playlist-modify-private user-follow-read user-follow-modify playlist-modify-private user-library-modify user-library-read")
        .build();

    match get_token(&mut oauth).await {
        Some(token_info) => {
            //TODO add structopt for file path
            let tidal = tidal::get_tidal_from_file(String::from("./tidal-tracks-mixed.json")).await?; //TODO add buffered reading
            println!("Importing {} tracks", tidal.items.len());

            println!("Getting spotify credentials");
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            let spotify = Spotify::default()
                .client_credentials_manager(client_credential)
                .build();

            println!("Getting user");
            let user = spotify.current_user().await.expect("Failed to get user");


            println!("Collecting tracks");
            let queries: Vec<(String, String)> = tidal.items.iter()
                .map(|track| {
                    let artist: String = track.item.artists.iter().map(|artist| artist.name.to_lowercase()).collect::<Vec<String>>().join(" ");
                    let query: String = vec![artist, track.item.title.to_lowercase()].join(" ");
                    (track.item.artist.name.to_lowercase(), query)
                }).collect();

            println!("Searching tracks");
            let mut search_results: Vec<(String, String, Result<SearchResult, _>)> = vec![];

            for (artist, query) in queries {
                let query = sanitize_query(query);
                let query_cloned = query.clone();
                let future = spotify.search(
                    query_cloned.as_str(),
                    SearchType::Track,
                    50,
                    0,
                    None,
                    None,
                );
                search_results.push((artist, query, future.await));
            }

            let mut track_uris = vec![];
            let mut failed = vec![];

            //TODO maybe use par it
            search_results.iter()
                .for_each(|(artist, query, find)| {
                    if let Ok(SearchResult::Tracks(tracks)) = find {
                        let tracks = tracks.items
                            .iter()
                            .filter(|track| {
                                let artists = build_track_artists(track);
                                artists.contains(&artist)
                            }).collect::<Vec<&FullTrack>>();
                        match tracks.first() {
                            None => {
                                println!("Could not find {} {}", artist, query); //TODO move me
                                failed.push(format!("{}", query));
                            }
                            Some(value) => {
                                let uri = value.uri.clone();
                                log::debug!("Found {} {:?}", query, uri); //TODO iterate elsewhere
                                track_uris.push(uri);
                            }
                        }
                    }
                });

            let futures = track_uris.chunks(99).map(|track_ids| {
                spotify.user_playlist_add_tracks(
                    user.id.as_str(),
                    test_import_playlist,
                    &track_ids,
                    None,
                )
            });

            let results = futures::future::join_all(futures).await;
            results.iter().for_each(|res| {
                match res {
                    Ok(result) => println!("Added {:?}", result),
                    Err(err) => println!("Failed to add because {}", err),
                }
            })
        }
        None => log::error!("Authentication failed"),
    };
    Ok(())
}

fn sanitize_query(query: String) -> String {
    let query = query.replace("(feat. ", "");
    let query = query.replace(")", "");
    query
}

fn build_track_artists(track: &FullTrack) -> Vec<String> {
    track.artists.iter().map(|artist| artist.name.to_lowercase()).collect::<Vec<String>>()
}