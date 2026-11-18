use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use glib::clone;
use gtk::{gio, glib};

use crate::config;
use crate::i18n::gettext_f;
use crate::recordings;

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/tech/vitainside/mirage/ui/preferences-dialog.ui")]
    pub struct MiragePreferencesDialog {
        #[template_child]
        pub video_codec_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub max_size_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub max_fps_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub bit_rate_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub hardware_decoding_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub new_display_row: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub new_display_size_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub new_display_dpi_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub flex_display_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub window_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub aspect_ratio_lock_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub render_fit_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub turn_screen_off_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub stay_awake_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub keyboard_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub mouse_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub gamepad_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub audio_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub audio_source_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub audio_codec_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub record_format_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub recordings_folder_row: TemplateChild<adw::ActionRow>,

        pub settings: OnceCell<gio::Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MiragePreferencesDialog {
        const NAME: &'static str = "MiragePreferencesDialog";
        type Type = super::MiragePreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(
                "preferences.open-recordings-folder",
                None,
                |dialog, _, _| {
                    dialog.open_recordings_folder();
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MiragePreferencesDialog {
        fn constructed(&self) {
            self.parent_constructed();
            let settings = config::settings();
            self.obj().bind_settings(&settings);
            self.settings.set(settings).unwrap();
        }
    }

    impl WidgetImpl for MiragePreferencesDialog {}
    impl AdwDialogImpl for MiragePreferencesDialog {}
    impl PreferencesDialogImpl for MiragePreferencesDialog {}
}

glib::wrapper! {
    pub struct MiragePreferencesDialog(ObjectSubclass<imp::MiragePreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl MiragePreferencesDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn bind_settings(&self, settings: &gio::Settings) {
        let imp = self.imp();

        // Mirroring
        bind_string_choice(
            settings,
            "video-codec",
            &imp.video_codec_row,
            vec![
                ("h264", "H.264".to_owned()),
                ("h265", "H.265 (HEVC)".to_owned()),
                ("av1", "AV1".to_owned()),
                ("vp8", "VP8".to_owned()),
                ("vp9", "VP9".to_owned()),
            ],
        );
        let mut sizes = vec![(0_u32, gettext("Original"))];
        sizes.extend([2560_u32, 1920, 1600, 1280, 1024, 800].map(|size| {
            // Translators: The size in pixels of the longest side of the video
            (size, gettext_f("{size} px", &[("size", &size.to_string())]))
        }));
        bind_choice(settings, "max-size", &imp.max_size_row, sizes);
        let mut rates = vec![(0_u32, gettext("Unlimited"))];
        rates.extend([30_u32, 60, 90, 120, 144].map(|fps| {
            // Translators: A frame rate, in frames per second
            (fps, gettext_f("{fps} FPS", &[("fps", &fps.to_string())]))
        }));
        bind_choice(settings, "max-fps", &imp.max_fps_row, rates);
        settings
            .bind("video-bit-rate", &imp.bit_rate_row.get(), "value")
            .build();
        bind_string_choice(
            settings,
            "hardware-decoding",
            &imp.hardware_decoding_row,
            vec![
                ("auto", pgettext("hardware decoding", "Automatic")),
                ("vaapi", "VA-API".to_owned()),
                ("disabled", pgettext("hardware decoding", "Disabled")),
            ],
        );

        settings
            .bind(
                "new-display",
                &imp.new_display_row.get(),
                "enable-expansion",
            )
            .build();
        let mut display_sizes = vec![(String::new(), gettext("Same as Device"))];
        display_sizes.extend(
            [
                (1280, 720),
                (1920, 1080),
                (2560, 1440),
                (3840, 2160),
                (1080, 1920),
            ]
            .map(|(width, height)| {
                (
                    format!("{width}x{height}"),
                    // Translators: A display resolution, like "1920 × 1080"
                    gettext_f(
                        "{width} × {height}",
                        &[
                            ("width", &width.to_string()),
                            ("height", &height.to_string()),
                        ],
                    ),
                )
            }),
        );
        bind_choice(
            settings,
            "new-display-size",
            &imp.new_display_size_row,
            display_sizes,
        );
        settings
            .bind("new-display-dpi", &imp.new_display_dpi_row.get(), "value")
            .build();
        setup_automatic_spin_row(&imp.new_display_dpi_row);
        settings
            .bind("flex-display", &imp.flex_display_row.get(), "active")
            .build();

        // Window
        settings
            .bind(
                "window-aspect-ratio-lock",
                &imp.aspect_ratio_lock_row.get(),
                "active",
            )
            .build();
        bind_string_choice(
            settings,
            "render-fit",
            &imp.render_fit_row,
            vec![
                ("letterbox", gettext("Fit")),
                ("stretched", gettext("Stretch")),
                ("unscaled", gettext("Original Size")),
            ],
        );
        // A display that follows the window always fills it.
        let update_window_group = clone!(
            #[weak(rename_to = group)]
            imp.window_group,
            move |settings: &gio::Settings, _: &str| {
                group.set_sensitive(
                    !(settings.boolean("new-display") && settings.boolean("flex-display")),
                );
            }
        );
        update_window_group(settings, "");
        settings.connect_changed(Some("new-display"), update_window_group.clone());
        settings.connect_changed(Some("flex-display"), update_window_group);

        settings
            .bind("turn-screen-off", &imp.turn_screen_off_row.get(), "active")
            .build();
        settings
            .bind("stay-awake", &imp.stay_awake_row.get(), "active")
            .build();

        // Input
        let input_modes = || {
            vec![
                ("sdk", gettext("Standard")),
                ("uhid", gettext("Hardware Emulation")),
                ("aoa", gettext("Hardware Emulation over USB")),
                ("disabled", pgettext("input device", "Disabled")),
            ]
        };
        bind_string_choice(settings, "keyboard", &imp.keyboard_row, input_modes());
        bind_string_choice(settings, "mouse", &imp.mouse_row, input_modes());
        bind_string_choice(
            settings,
            "gamepad",
            &imp.gamepad_row,
            vec![
                ("disabled", pgettext("input device", "Disabled")),
                ("uhid", gettext("Hardware Emulation")),
                ("aoa", gettext("Hardware Emulation over USB")),
            ],
        );

        // Audio
        settings
            .bind("audio", &imp.audio_row.get(), "active")
            .build();
        bind_string_choice(
            settings,
            "audio-source",
            &imp.audio_source_row,
            vec![
                ("output", gettext("Device Output")),
                ("playback", gettext("App Playback")),
                ("mic", gettext("Microphone")),
            ],
        );
        bind_string_choice(
            settings,
            "audio-codec",
            &imp.audio_codec_row,
            vec![
                ("opus", "Opus".to_owned()),
                ("aac", "AAC".to_owned()),
                ("flac", "FLAC".to_owned()),
                ("raw", gettext("Uncompressed")),
            ],
        );

        // Recording
        bind_string_choice(
            settings,
            "record-format",
            &imp.record_format_row,
            vec![("mp4", "MP4".to_owned()), ("mkv", "Matroska".to_owned())],
        );
        imp.recordings_folder_row
            .set_subtitle(&recordings::display_path(&recordings::recordings_dir()));
    }

    fn open_recordings_folder(&self) {
        let dir = recordings::recordings_dir();
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.add_toast(adw::Toast::new(&error.to_string()));
            return;
        }
        let file = gio::File::for_path(&dir);
        let root = self.root().and_downcast::<gtk::Window>();
        gtk::FileLauncher::new(Some(&file)).launch(
            root.as_ref(),
            gio::Cancellable::NONE,
            |result| {
                if let Err(error) = result {
                    tracing::warn!("Could not open the recordings folder: {error}");
                }
            },
        );
    }
}

impl Default for MiragePreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}

fn bind_string_choice(
    settings: &gio::Settings,
    key: &str,
    row: &adw::ComboRow,
    choices: Vec<(&str, String)>,
) {
    let choices = choices
        .into_iter()
        .map(|(value, label)| (value.to_owned(), label))
        .collect();
    bind_choice::<String>(settings, key, row, choices);
}

/// Binds a combo row to a setting that takes one of a fixed list of values.
fn bind_choice<T>(
    settings: &gio::Settings,
    key: &str,
    row: &adw::ComboRow,
    choices: Vec<(T, String)>,
) where
    T: ToVariant + FromVariant + PartialEq + Send + Sync + 'static,
{
    let labels: Vec<&str> = choices.iter().map(|(_, label)| label.as_str()).collect();
    row.set_model(Some(&gtk::StringList::new(&labels)));

    let values: Arc<Vec<T>> = Arc::new(choices.into_iter().map(|(value, _)| value).collect());
    settings
        .bind(key, row, "selected")
        .mapping({
            let values = values.clone();
            move |variant, _| {
                let value = variant.get::<T>()?;
                let position = values
                    .iter()
                    .position(|candidate| *candidate == value)
                    .map_or(gtk::INVALID_LIST_POSITION, |position| position as u32);
                Some(position.to_value())
            }
        })
        .set_mapping(move |value, _| {
            let position = value.get::<u32>().ok()?;
            values.get(position as usize).map(ToVariant::to_variant)
        })
        .build();
}

/// Shows "Automatic" in a spin row when its value is 0.
fn setup_automatic_spin_row(row: &adw::SpinRow) {
    row.connect_output(|row| {
        if row.value() == 0.0 {
            row.set_text(&pgettext("pixel density", "Automatic"));
            true
        } else {
            false
        }
    });
    row.connect_input(|row| {
        if row.text() == pgettext("pixel density", "Automatic") {
            Some(Ok(0.0))
        } else {
            None
        }
    });
}
