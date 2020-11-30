use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::Error;
use rspotify::client::Spotify;
use rspotify::model::cud_result::CUDResult;
use rspotify::model::search::SearchResult;
use rspotify::model::track::FullTrack;
use rspotify::model::user::PrivateUser;
use rspotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo};
use rspotify::senum::{Country, IncludeExternal, SearchType};
use rspotify::util::get_token;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;

use crate::cli::Opts;
use crate::provider::StreamingProvider;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tidal {
    pub limit: i64,
    pub offset: i64,
    pub total_number_of_items: i64,
    pub items: Vec<Track>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub item: TrackDetails,
    #[serde(rename = "type")]
    pub type_field: String,
    pub cut: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetails {
    pub id: i64,
    pub title: String,
    pub duration: i64,
    pub replay_gain: f64,
    pub peak: f64,
    pub allow_streaming: bool,
    pub stream_ready: bool,
    pub stream_start_date: String,
    pub premium_streaming_only: bool,
    pub track_number: i64,
    pub volume_number: i64,
    pub version: Option<String>,
    pub popularity: i64,
    pub copyright: String,
    pub description: Value,
    pub url: String,
    pub isrc: String,
    pub editable: bool,
    pub explicit: bool,
    pub audio_quality: String,
    pub audio_modes: Vec<String>,
    pub artist: Artist,
    pub artists: Vec<Artist2>,
    pub album: Album,
    pub mixes: Mixes,
    pub date_added: String,
    pub index: i64,
    pub item_uuid: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist2 {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub cover: Option<String>,
    pub video_cover: Value,
    pub release_date: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mixes {
    #[serde(rename = "TRACK_MIX")]
    pub track_mix: Option<String>,
    #[serde(rename = "MASTER_TRACK_MIX")]
    pub master_track_mix: Option<String>,
}

pub struct TidalProvider {
    pub playlist: String,
    pub file: PathBuf,
}

impl TidalProvider {
    pub fn new(opts: &Opts) -> TidalProvider {
        TidalProvider {
            playlist: opts.playlist.to_owned(),
            file: opts.file.to_owned(),
        }
    }
}

#[async_trait]
impl StreamingProvider for TidalProvider {
    async fn import(&self, spotify: Spotify, user: PrivateUser) -> Result<(), Error> {
        println!("> Reading tidal file..");
        let tidal = get_tidal_from_file(&self.file).await?;
        println!("> Importing {} tracks..", tidal.items.len() - 1);
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
        let mut failed_uris = vec![];

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
                            let message = format!("Could not find {} {}", artist, query);
                            failed_uris.push(message);
                        }
                        Some(value) => {
                            let uri = value.uri.clone();
                            log::debug!("Found {} {:?}", query, uri);
                            track_uris.push(uri);
                        }
                    }
                }
            });

        failed_uris.iter().for_each(|message| log::debug!("{}", message));


        let mut results = vec![];
        //TODO at this point we should probably retry
        let mut futures = track_uris.chunks(80);
        while let Some(track_ids) = futures.next() {
            results.push(spotify.user_playlist_add_tracks(
                user.id.as_str(),
                self.playlist.as_str(),
                &track_ids,
                None,
            ).await);
        }

        results.iter().for_each(|res| {
            match res {
                Ok(result) => println!("Added {:?}", result),
                Err(err) => println!("Failed to add because {}", err),
            }
        });

        //TODO dont do this
        Ok(())
    }
}

pub async fn get_tidal_from_file(path: &PathBuf) -> Result<Tidal, Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let result: Result<Tidal, _> = serde_json::from_reader(reader);
    match result {
        Ok(val) => Ok(val),
        Err(err) => Err(anyhow::anyhow!(format!("Some issue {}", err)))
    }
}

fn sanitize_query(query: String) -> String {
    let query = query.replace("(feat. ", "");
    let query = query.replace(")", "");
    query
}


fn build_track_artists(track: &FullTrack) -> Vec<String> {
    track.artists.iter().map(|artist| artist.name.to_lowercase()).collect::<Vec<String>>()
}