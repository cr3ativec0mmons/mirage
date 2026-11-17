//! A QR code drawn with sharp edges at any size and display scale.

use std::cell::OnceCell;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

/// The size of the code on screen, in logical pixels.
const SIZE: i32 = 224;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct QrCodePaintable {
        /// One pixel per module.
        pub texture: OnceCell<gdk::Texture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QrCodePaintable {
        const NAME: &'static str = "MirageQrCodePaintable";
        type Type = super::QrCodePaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for QrCodePaintable {}

    impl PaintableImpl for QrCodePaintable {
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE | gdk::PaintableFlags::STATIC_CONTENTS
        }

        fn intrinsic_width(&self) -> i32 {
            SIZE
        }

        fn intrinsic_height(&self) -> i32 {
            SIZE
        }

        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let Some(texture) = self.texture.get() else {
                return;
            };
            let snapshot = snapshot.downcast_ref::<gtk::Snapshot>().unwrap();
            snapshot.append_scaled_texture(
                texture,
                gsk::ScalingFilter::Nearest,
                &graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
            );
        }
    }
}

glib::wrapper! {
    pub struct QrCodePaintable(ObjectSubclass<imp::QrCodePaintable>)
        @implements gdk::Paintable;
}

impl QrCodePaintable {
    pub fn new(payload: &str) -> Option<Self> {
        let code = qrcode::QrCode::new(payload).ok()?;
        let width = code.width();
        let pixels: Vec<u8> = code
            .to_colors()
            .into_iter()
            .map(|color| {
                if color == qrcode::Color::Dark {
                    0
                } else {
                    0xff
                }
            })
            .collect();
        let texture = gdk::MemoryTexture::new(
            width as i32,
            width as i32,
            gdk::MemoryFormat::G8,
            &glib::Bytes::from_owned(pixels),
            width,
        );

        let paintable: Self = glib::Object::new();
        paintable.imp().texture.set(texture.upcast()).unwrap();
        Some(paintable)
    }
}
