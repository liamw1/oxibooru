use image::{DynamicImage, ImageFormat, ImageReader, ImageResult, Limits};
use mp4::{Mp4Reader, TrackType};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum VideoDecodingError {
    Mp4(#[from] mp4::Error),
    #[error("Video file does not have a video track")]
    NoVideoTrack,
}

/*
    Decodes a raw array of bytes into pixel data.
*/
pub fn image(bytes: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(image_reader_limits());
    reader.decode()
}

pub fn video_dimensions(path: &Path) -> Result<(u16, u16), VideoDecodingError> {
    let file = File::open(path).map_err(mp4::Error::IoError)?;
    let file_size = file.metadata().map_err(mp4::Error::IoError)?.len();
    let file_reader = BufReader::new(file);

    let mp4 = Mp4Reader::read_header(file_reader, file_size)?;
    let tracks: Vec<_> = mp4
        .tracks()
        .values()
        .map(|track| track.track_type().map(|track_type| (track, track_type)))
        .collect::<Result<_, _>>()?;
    let video_track = tracks
        .into_iter()
        .find(|(_, track_type)| *track_type == TrackType::Video)
        .map(|(track, _)| track)
        .ok_or(VideoDecodingError::NoVideoTrack)?;

    Ok((video_track.width(), video_track.height()))
}

fn image_reader_limits() -> Limits {
    const GB: u64 = 1024 * 1024 * 1024;

    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(4 * GB);
    limits
}
