use super::*;

#[test]
fn parse_h264_encoders_reads_the_encoders_listing() {
    let listing = "Encoders:\n V..... = Video\n A..... = Audio\n ------\n \
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)\n \
 V....D libx264rgb           libx264 H.264 / AVC RGB (codec h264)\n \
 V....D libopenh264          OpenH264 H.264 / AVC (codec h264)\n \
 V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)\n \
 V....D h264_mf              H264 via MediaFoundation (codec h264)\n \
 V....D mpeg4                MPEG-4 part 2\n \
 A....D aac                  AAC (Advanced Audio Coding)\n";
    assert_eq!(
        parse_h264_encoders(listing),
        ["libopenh264", "libx264", "h264_mf", "h264_videotoolbox"]
    );
    assert!(parse_h264_encoders("Encoders:\n ------\n A....D aac  AAC\n").is_empty());
}
