# Visual tokens in this build

These are the numbers this Spotlight pill actually uses. They are recorded so a later shape pass can reuse them. They are not a website theme. This tree does not change a website and does not import a separate theme package. The menu keeps the configured Omarchy font.

| Token | Value |
|---|---|
| Logical width | 680 |
| Idle search control | 54 |
| Idle window | 74, which is the search control plus 10px padding |
| Maximum height | 440 |
| Anchor | centered on the focused output's full logical rectangle; grows down from that center, clamped inside the output |
| Corner radius | 18 |
| Panel background alpha | 196, premultiplied |
| Shell | one rounded plate. The search field is text in that plate, not a second slab |
| Focus | 1px border. Not a filled inner pill |
| Glyph interior | alpha 255 |
| Mark | two short bars, accent color, no name and no portrait |
| Rows | one column, up to 8, fewer when the font or the height limit says so |
| Confirm | Cancel is the default. Confirm is the second control, inline under the copy |

Wayland draws that panel on an ARGB layer surface named `omarchy-menu-rust`. X11 flattens the same pixels to an opaque window and says so. A click outside the panel is still delivered to the window underneath. That limit is open.
