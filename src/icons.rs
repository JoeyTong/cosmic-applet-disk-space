use crate::storage::DriveKind;
use cosmic::widget::icon::{Handle, from_svg_bytes};

pub fn drive(kind: DriveKind) -> Handle {
    let bytes: &'static [u8] = match kind {
        DriveKind::InternalSsd => include_bytes!("../data/icons/internal-ssd.svg"),
        DriveKind::InternalHdd => include_bytes!("../data/icons/internal-hdd.svg"),
        DriveKind::ExternalSsd => include_bytes!("../data/icons/external-ssd.svg"),
        DriveKind::ExternalHdd => include_bytes!("../data/icons/external-hdd.svg"),
        DriveKind::UsbFlash => include_bytes!("../data/icons/usb-flash.svg"),
        DriveKind::SdCard => include_bytes!("../data/icons/sd-card.svg"),
        DriveKind::Unknown => include_bytes!("../data/icons/drive.svg"),
    };
    from_svg_bytes(bytes).symbolic(true)
}

pub fn warning() -> Handle {
    from_svg_bytes(include_bytes!("../data/icons/warning.svg").as_slice()).symbolic(true)
}
