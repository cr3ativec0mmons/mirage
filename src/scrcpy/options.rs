use std::ffi::OsString;
use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;

/// The settings that are passed to scrcpy as command line options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub video_codec: String,
    /// The largest dimension of the video, or 0 for the device resolution.
    pub max_size: u32,
    /// 0 means no limit.
    pub max_fps: u32,
    /// In megabits per second.
    pub video_bit_rate: u32,
    pub hardware_decoding: String,
    pub new_display: Option<NewDisplay>,
    pub turn_screen_off: bool,
    pub stay_awake: bool,
    pub keyboard: String,
    pub mouse: String,
    pub gamepad: String,
    pub audio: Option<Audio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDisplay {
    /// `WIDTHxHEIGHT`, or empty to match the main display.
    pub size: String,
    /// 0 to match the main display.
    pub dpi: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audio {
    pub codec: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    pub path: PathBuf,
    pub format: String,
}

impl Default for Options {
    /// The defaults of scrcpy, which are also those of the settings schema.
    fn default() -> Self {
        Self {
            video_codec: "h264".to_owned(),
            max_size: 0,
            max_fps: 0,
            video_bit_rate: 8,
            hardware_decoding: "auto".to_owned(),
            new_display: None,
            turn_screen_off: false,
            stay_awake: false,
            keyboard: "sdk".to_owned(),
            mouse: "sdk".to_owned(),
            gamepad: "disabled".to_owned(),
            audio: Some(Audio {
                codec: "opus".to_owned(),
                source: "output".to_owned(),
            }),
        }
    }
}

impl Options {
    pub fn from_settings(settings: &gio::Settings) -> Self {
        let string = |key| settings.string(key).to_string();
        Self {
            video_codec: string("video-codec"),
            max_size: settings.uint("max-size"),
            max_fps: settings.uint("max-fps"),
            video_bit_rate: settings.uint("video-bit-rate"),
            hardware_decoding: string("hardware-decoding"),
            new_display: settings.boolean("new-display").then(|| NewDisplay {
                size: string("new-display-size"),
                dpi: settings.uint("new-display-dpi"),
            }),
            turn_screen_off: settings.boolean("turn-screen-off"),
            stay_awake: settings.boolean("stay-awake"),
            keyboard: string("keyboard"),
            mouse: string("mouse"),
            gamepad: string("gamepad"),
            audio: settings.boolean("audio").then(|| Audio {
                codec: string("audio-codec"),
                source: string("audio-source"),
            }),
        }
    }

    /// The command line arguments of scrcpy, without the program name.
    pub fn to_args(
        &self,
        serial: &str,
        window_title: &str,
        recording: Option<&Recording>,
    ) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            format!("--serial={serial}").into(),
            format!("--window-title={window_title}").into(),
            format!("--video-codec={}", self.video_codec).into(),
            format!("--video-bit-rate={}M", self.video_bit_rate).into(),
            format!("--hwdec={}", self.hardware_decoding).into(),
        ];

        if self.max_size > 0 {
            args.push(format!("--max-size={}", self.max_size).into());
        }
        if self.max_fps > 0 {
            args.push(format!("--max-fps={}", self.max_fps).into());
        }

        if let Some(display) = &self.new_display {
            let mut arg = String::from("--new-display");
            if !display.size.is_empty() || display.dpi > 0 {
                arg.push('=');
                arg.push_str(&display.size);
                if display.dpi > 0 {
                    arg.push_str(&format!("/{}", display.dpi));
                }
            }
            args.push(arg.into());
        }

        if self.turn_screen_off {
            args.push("--turn-screen-off".into());
        }
        if self.stay_awake {
            args.push("--stay-awake".into());
        }

        args.push(format!("--keyboard={}", self.keyboard).into());
        args.push(format!("--mouse={}", self.mouse).into());
        args.push(format!("--gamepad={}", self.gamepad).into());

        match &self.audio {
            Some(audio) => {
                args.push(format!("--audio-codec={}", audio.codec).into());
                args.push(format!("--audio-source={}", audio.source).into());
            }
            None => args.push("--no-audio".into()),
        }

        if let Some(recording) = recording {
            let mut arg = OsString::from("--record=");
            arg.push(&recording.path);
            args.push(arg);
            args.push(format!("--record-format={}", recording.format).into());
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(options: &Options, recording: Option<&Recording>) -> Vec<String> {
        options
            .to_args("ABC123", "Pixel 8", recording)
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn default_options() {
        assert_eq!(
            args(&Options::default(), None),
            [
                "--serial=ABC123",
                "--window-title=Pixel 8",
                "--video-codec=h264",
                "--video-bit-rate=8M",
                "--hwdec=auto",
                "--keyboard=sdk",
                "--mouse=sdk",
                "--gamepad=disabled",
                "--audio-codec=opus",
                "--audio-source=output",
            ]
        );
    }

    #[test]
    fn limits_and_device_options() {
        let options = Options {
            max_size: 1920,
            max_fps: 60,
            turn_screen_off: true,
            stay_awake: true,
            audio: None,
            ..Options::default()
        };
        let args = args(&options, None);
        for expected in [
            "--max-size=1920",
            "--max-fps=60",
            "--turn-screen-off",
            "--stay-awake",
            "--no-audio",
        ] {
            assert!(args.iter().any(|arg| arg == expected), "missing {expected}");
        }
        assert!(!args.iter().any(|arg| arg.starts_with("--audio-codec")));
    }

    #[test]
    fn new_display_variants() {
        let new_display = |size: &str, dpi| {
            let options = Options {
                new_display: Some(NewDisplay {
                    size: size.to_owned(),
                    dpi,
                }),
                ..Options::default()
            };
            args(&options, None)
                .into_iter()
                .find(|arg| arg.starts_with("--new-display"))
                .unwrap()
        };
        assert_eq!(new_display("", 0), "--new-display");
        assert_eq!(new_display("1920x1080", 0), "--new-display=1920x1080");
        assert_eq!(new_display("", 240), "--new-display=/240");
        assert_eq!(new_display("1920x1080", 420), "--new-display=1920x1080/420");
    }

    #[test]
    fn recording() {
        let recording = Recording {
            path: PathBuf::from("/home/user/Videos/Mirage/Pixel 8.mp4"),
            format: "mp4".to_owned(),
        };
        let args = args(&Options::default(), Some(&recording));
        assert!(args.contains(&"--record=/home/user/Videos/Mirage/Pixel 8.mp4".to_owned()));
        assert!(args.contains(&"--record-format=mp4".to_owned()));
    }
}
