use base64::encode;

const MAGIC: [(&[u8], &str); 19] = [
    // Image
    (b"GIF87a", "image/gif"),
    (b"GIF89a", "image/gif"),
    (b"\xFF\xD8\xFF", "image/jpeg"),
    (b"\x89PNG\x0D\x0A\x1A\x0A", "image/png"),
    (b"<?xml ", "image/svg+xml"),
    (b"<svg ", "image/svg+xml"),
    (b"RIFF....WEBPVP8 ", "image/webp"),
    (b"\x00\x00\x01\x00", "image/x-icon"),
    // Audio
    (b"ID3", "audio/mpeg"),
    (b"\xFF\x0E", "audio/mpeg"),
    (b"\xFF\x0F", "audio/mpeg"),
    (b"OggS", "audio/ogg"),
    (b"RIFF....WAVEfmt ", "audio/wav"),
    (b"fLaC", "audio/x-flac"),
    // Video
    (b"RIFF....AVI LIST", "video/avi"),
    (b"....ftyp", "video/mp4"),
    (b"\x00\x00\x01\x0B", "video/mpeg"),
    (b"....moov", "video/quicktime"),
    (b"\x1A\x45\xDF\xA3", "video/webm"),
];

pub fn detect_mimetype(data: &[u8]) -> &str {
    for (magic_bytes, mime) in MAGIC.iter() {
        if data.starts_with(magic_bytes) {
            return mime;
        }
    }
    ""
}

pub fn data_to_dataurl(mime: &str, data: &[u8]) -> String {
    let mimetype = if mime.is_empty() {
        detect_mimetype(data)
    } else {
        mime
    };
    format!("data:{};base64,{}", mimetype, encode(data))
}
