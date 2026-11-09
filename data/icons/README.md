# Disk Space icons

The drive icons use the approved **COSMIC-based set C**. All SVGs have a 24×24 viewBox and are embedded as symbolic icons, so libcosmic applies the current theme or warning color.

| Applet asset | Design / upstream source |
| --- | --- |
| `internal-ssd.svg` | COSMIC `drive-harddisk-solidstate-symbolic.svg`: solid SSD enclosure and indicator |
| `internal-hdd.svg` | COSMIC `drive-harddisk-system-symbolic.svg`: enclosure with platter |
| `external-ssd.svg` | COSMIC SSD enclosure with a custom cable and connector |
| `external-hdd.svg` | COSMIC HDD enclosure with a custom cable and connector |
| `usb-flash.svg` | COSMIC `drive-harddisk-usb-symbolic.svg`: USB symbol |
| `sd-card.svg` | COSMIC `media-flash-symbolic.svg`: memory-card silhouette and contacts |
| `drive.svg` | Original neutral drive enclosure; also used for the applet launcher |
| `warning.svg` | Original warning symbol |

## Attribution and license

“[Cosmic Icons](https://github.com/pop-os/cosmic-icons)” by [System76](https://system76.com/) is licensed under [Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)](https://creativecommons.org/licenses/by-sa/4.0/).

The six COSMIC-derived assets above come from the `epoch-1.11.0` release: `extra/scalable/devices` for the SSD, HDD and USB symbols, and `freedesktop/scalable/devices` for the memory card. The adapted assets, including the added external-drive cables, are also licensed under **CC BY-SA 4.0**. Changes made for this applet: scale to a 24×24 canvas, normalize fills for symbolic rendering, remove redundant transparent paths/clipping metadata, and add cables/connectors to the external variants.

- Source release: https://github.com/pop-os/cosmic-icons/tree/epoch-1.11.0
- Upstream license: https://github.com/pop-os/cosmic-icons/blob/epoch-1.11.0/LICENSE
- Full license terms and warranty disclaimer: https://creativecommons.org/licenses/by-sa/4.0/legalcode.en

The original `drive.svg` and `warning.svg` assets are not derived from Cosmic Icons.
