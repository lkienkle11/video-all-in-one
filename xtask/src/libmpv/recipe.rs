//! The configure and meson arguments of every component. They are plain data so that the LGPL
//! guarantees (no GPL, no non-free, no external codec libraries) can be tested on any machine.
//! The option names were checked against the pinned source archives (see `sources.rs`).

use super::Target;

/// FFmpeg: LGPL, shared, no programs, only the native decoders.
pub(super) fn ffmpeg_configure(target: Target, prefix: &str) -> Vec<String> {
    let mut args = strings(&[
        "--disable-gpl",
        "--disable-nonfree",
        "--enable-shared",
        "--disable-static",
        "--disable-programs",
        "--disable-doc",
        "--disable-avdevice",
        // Nothing outside FFmpeg is linked unless it is named below.
        "--disable-autodetect",
        "--enable-zlib",
        "--enable-pic",
    ]);
    args.push(format!("--prefix={prefix}"));
    if target == Target::Macos {
        // Hardware decoding through the system frameworks (no external library).
        args.push("--enable-videotoolbox".into());
        args.push("--enable-audiotoolbox".into());
    }
    args
}

/// FreeType without any optional library (no PNG, bzip2, brotli, HarfBuzz or system zlib).
pub(super) fn freetype_configure(prefix: &str) -> Vec<String> {
    let mut args = strings(&[
        "--enable-shared",
        "--disable-static",
        "--with-zlib=no",
        "--with-bzip2=no",
        "--with-png=no",
        "--with-harfbuzz=no",
        "--with-brotli=no",
    ]);
    args.push(format!("--prefix={prefix}"));
    args
}

/// GNU FriBidi.
pub(super) fn fribidi_configure(prefix: &str) -> Vec<String> {
    let mut args = strings(&["--enable-shared", "--disable-static"]);
    args.push(format!("--prefix={prefix}"));
    args
}

/// HarfBuzz with FreeType and nothing else.
pub(super) fn harfbuzz_meson(prefix: &str) -> Vec<String> {
    let mut args = meson_base(prefix);
    args.extend(strings(&[
        "-Dfreetype=enabled",
        "-Dglib=disabled",
        "-Dgobject=disabled",
        "-Dcairo=disabled",
        "-Dchafa=disabled",
        "-Dicu=disabled",
        "-Dcoretext=disabled",
        "-Dtests=disabled",
        "-Dintrospection=disabled",
        "-Ddocs=disabled",
        "-Dutilities=disabled",
    ]));
    args
}

/// libass. The app draws no subtitles from system fonts, so no font provider is required
/// (fontconfig is not a dependency of the Ubuntu package).
pub(super) fn libass_configure(target: Target, prefix: &str) -> Vec<String> {
    let mut args = strings(&[
        "--enable-shared",
        "--disable-static",
        "--disable-libunibreak",
        "--disable-require-system-font-provider",
    ]);
    if target == Target::Ubuntu {
        args.push("--disable-fontconfig".into());
    }
    args.push(format!("--prefix={prefix}"));
    args
}

/// libplacebo without any GPU API: mpv needs the library to build, and the app renders in
/// software.
pub(super) fn libplacebo_meson(prefix: &str) -> Vec<String> {
    let mut args = meson_base(prefix);
    args.extend(strings(&[
        "-Dvulkan=disabled",
        "-Dopengl=disabled",
        "-Dd3d11=disabled",
        "-Dglslang=disabled",
        "-Dshaderc=disabled",
        "-Dlcms=disabled",
        "-Ddovi=disabled",
        "-Dlibdovi=disabled",
        "-Dunwind=disabled",
        "-Dxxhash=disabled",
        "-Ddemos=false",
        "-Dtests=false",
        "-Dbench=false",
        "-Dfuzz=false",
    ]));
    args
}

/// libmpv, LGPL only, no player program, software render API, every optional feature off
/// except the audio output of the platform.
pub(super) fn mpv_meson(target: Target, prefix: &str) -> Vec<String> {
    let mut args = meson_base(prefix);
    args.extend(strings(&[
        "-Dgpl=false",
        "-Dlibmpv=true",
        "-Dcplayer=false",
        "-Dbuild-date=false",
        "-Dtests=false",
        "-Dmanpage-build=disabled",
        "-Dlua=disabled",
        "-Djavascript=disabled",
        "-Dcplugins=disabled",
        "-Dcdda=disabled",
        "-Ddvdnav=disabled",
        "-Ddvbin=disabled",
        "-Djpeg=disabled",
        "-Dlcms2=disabled",
        "-Dlibarchive=disabled",
        "-Dlibavdevice=disabled",
        "-Dlibbluray=disabled",
        "-Drubberband=disabled",
        "-Dsdl2-gamepad=disabled",
        "-Dsdl2-audio=disabled",
        "-Dsdl2-video=disabled",
        "-Duchardet=disabled",
        "-Dvapoursynth=disabled",
        "-Dzimg=disabled",
        // No video output window and no GPU API: the app uses the software render API.
        "-Dgl=disabled",
        "-Dplain-gl=disabled",
        "-Degl=disabled",
        "-Dvulkan=disabled",
        "-Dshaderc=disabled",
        "-Dspirv-cross=disabled",
        "-Dx11=disabled",
        "-Dwayland=disabled",
        "-Dxv=disabled",
        "-Ddrm=disabled",
        "-Dgbm=disabled",
        "-Dvdpau=disabled",
        "-Dvaapi=disabled",
        "-Dcaca=disabled",
        "-Dsixel=disabled",
        "-Dcocoa=disabled",
        "-Dgl-cocoa=disabled",
        "-Dswift-build=disabled",
        "-Dmacos-cocoa-cb=disabled",
        "-Dmacos-media-player=disabled",
        "-Dmacos-touchbar=disabled",
        "-Dvideotoolbox-gl=disabled",
        "-Dvideotoolbox-pl=disabled",
        // Audio outputs.
        "-Djack=disabled",
        "-Dopenal=disabled",
        "-Dsndio=disabled",
        "-Doss-audio=disabled",
        "-Dpipewire=disabled",
    ]));
    let (on, off) = match target {
        Target::Ubuntu => (["alsa", "pulse"].as_slice(), ["coreaudio"].as_slice()),
        Target::Macos => (["coreaudio"].as_slice(), ["alsa", "pulse"].as_slice()),
    };
    for name in on {
        args.push(format!("-D{name}=enabled"));
    }
    for name in off {
        args.push(format!("-D{name}=disabled"));
    }
    args
}

/// Options every meson component gets.
fn meson_base(prefix: &str) -> Vec<String> {
    let mut args = strings(&[
        "--buildtype=release",
        "--libdir=lib",
        "-Ddefault_library=shared",
    ]);
    args.push(format!("--prefix={prefix}"));
    args
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(args: &[String], wanted: &str) -> bool {
        args.iter().any(|arg| arg == wanted)
    }

    #[test]
    fn ffmpeg_is_lgpl_shared_and_links_no_external_codec_library() {
        for target in [Target::Ubuntu, Target::Macos] {
            let args = ffmpeg_configure(target, "/p");
            for wanted in [
                "--disable-gpl",
                "--disable-nonfree",
                "--enable-shared",
                "--disable-static",
                "--disable-programs",
                "--disable-doc",
                "--disable-autodetect",
                "--prefix=/p",
            ] {
                assert!(has(&args, wanted), "{target:?} lacks {wanted}");
            }
            for arg in &args {
                assert!(
                    !matches!(arg.as_str(), "--enable-gpl" | "--enable-nonfree"),
                    "{arg}"
                );
                assert!(
                    !arg.starts_with("--enable-lib"),
                    "no external library may be linked: {arg}"
                );
            }
        }
        assert!(has(
            &ffmpeg_configure(Target::Macos, "/p"),
            "--enable-videotoolbox"
        ));
        assert!(!has(
            &ffmpeg_configure(Target::Ubuntu, "/p"),
            "--enable-videotoolbox"
        ));
    }

    #[test]
    fn mpv_is_lgpl_libmpv_only_with_the_audio_output_of_its_platform() {
        let linux = mpv_meson(Target::Ubuntu, "/p");
        for wanted in [
            "-Dgpl=false",
            "-Dlibmpv=true",
            "-Dcplayer=false",
            "-Dlua=disabled",
            "-Djavascript=disabled",
            "-Dalsa=enabled",
            "-Dpulse=enabled",
            "-Dcoreaudio=disabled",
            "--prefix=/p",
        ] {
            assert!(has(&linux, wanted), "linux lacks {wanted}");
        }
        let mac = mpv_meson(Target::Macos, "/p");
        assert!(has(&mac, "-Dcoreaudio=enabled"));
        assert!(has(&mac, "-Dalsa=disabled") && has(&mac, "-Dpulse=disabled"));
        assert!(has(&mac, "-Dgpl=false"));
        assert!(!mac.iter().any(|arg| arg == "-Dgpl=true"));
    }

    #[test]
    fn no_component_is_asked_for_a_gpl_option_or_a_system_codec() {
        let all = [
            ffmpeg_configure(Target::Ubuntu, "/p"),
            freetype_configure("/p"),
            fribidi_configure("/p"),
            harfbuzz_meson("/p"),
            libass_configure(Target::Ubuntu, "/p"),
            libplacebo_meson("/p"),
            mpv_meson(Target::Ubuntu, "/p"),
        ];
        for args in &all {
            for arg in args {
                assert!(!arg.contains("gpl=true"), "{arg}");
                assert!(!arg.contains("x264") && !arg.contains("x265"), "{arg}");
                assert!(!arg.contains("fdk") && !arg.contains("xvid"), "{arg}");
            }
            assert!(
                args.iter().any(|arg| arg == "--prefix=/p"),
                "every component installs into the same prefix"
            );
        }
    }
}
