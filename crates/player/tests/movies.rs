//! Decodes each movie of a real import folder.
//!
//! The repository holds no game data. Set `DLGE_IMPORT` to a folder that
//! `dlplayer import` wrote. Without the variable, the test does nothing.

use std::path::PathBuf;

use player::movie::{Movie, decode_sound};

#[test]
fn every_movie_decodes() {
    let Some(dir) = std::env::var_os("DLGE_IMPORT").map(PathBuf::from) else {
        eprintln!("DLGE_IMPORT is not set. The movie test does nothing.");
        return;
    };
    let mut movies: Vec<PathBuf> = std::fs::read_dir(dir.join("movies"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    movies.sort();
    assert!(!movies.is_empty());
    for path in movies {
        // The test decodes the first 10 seconds of each movie.
        let length = 10.0f64;
        let mut movie = Movie::open(&path).unwrap();
        let mut pictures = 0;
        let mut t = 0.0;
        while !movie.finished() && t < length {
            t += 1.0 / 30.0;
            if movie.advance(t).unwrap() {
                pictures += 1;
            }
        }
        let expected = (movie.duration().min(length) * 30.0) as usize;
        let sound = decode_sound(&path).unwrap();
        let seconds = sound
            .as_ref()
            .map(|s| s.samples.len() as f64 / s.channels as f64 / s.rate as f64);
        eprintln!(
            "{}: {}x{}, {pictures} of {expected} pictures, {:.1} s of video, sound {seconds:.1?} s",
            path.file_name().unwrap().to_string_lossy(),
            movie.width,
            movie.height,
            movie.duration(),
        );
        assert_eq!((movie.width, movie.height), (1920, 1080));
        // A decoder with B-frames keeps a few pictures back.
        assert!(pictures + 8 >= expected, "{pictures} of {expected}");
        if let Some(seconds) = seconds {
            assert!((seconds - movie.duration()).abs() < 1.0);
        }
    }
}
