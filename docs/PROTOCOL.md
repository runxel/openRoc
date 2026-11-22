# ROCCAT Kone XP – HID configuration protocol

Working notes from reverse engineering ROCCAT Swarm 1.9388 (`roccat_swarm_monitor.exe`,
classes `kone_xp_base_c` / `kone_xp_device_c` / `kone_xp_profile_c`) and from reading a
live device (firmware 1.09).

Each statement is tagged with how sure it is:

- **[C]** confirmed: seen in the decompiled code *and* verified against the live mouse
- **[D]** decompiled only: seen in the code, not yet exercised on the device
- **[I]** inferred from the data of the five stored profiles
- **[?]** unknown

## Device

| | |
|---|---|
| USB ID | `1e7d:2c8b` (ROCCAT Kone XP, wired) |
| Config interface | USB interface 3, HID usage page `0xFF01`, usage `0x01` |
| Transport | HID feature reports only (GET_FEATURE / SET_FEATURE) |
| Other interfaces | 0: mouse, 1: vendor `0xFF00` 64-byte in/out (live lighting), 2: keyboard/consumer + vendor `0xFF02` event report |

No kernel driver is needed. On macOS the `0xFF01` interface opens through IOHIDManager
(hidapi) without Input Monitoring permission. **[C]**

## Feature reports on the config interface

Sizes include the report ID byte.

| ID | Size | Purpose | |
|---|---|---|---|
| `0x04` | 4 | Control: select what to read, and write status | [C] |
| `0x05` | 4 | Profile info: active profile and profile count | [C] |
| `0x06` | 174 | Profile settings (DPI, lighting, sensor) | [C] |
| `0x07` | 125 | Profile buttons (30 slots) | [C] |
| `0x08` | 1043 | Macro for one button slot | [D] |
| `0x09` | 9 | Device info, firmware version | [C] |
| `0x0e` | 6 | Lighting software control (`0e 06 a b 00 ff`) | [D] |
| `0x0f` | 6 | Device-wide setting, see below (probably lift-off distance) | [D], meaning [I] |
| `0x11` | 20 | Device-wide setting, see below (debounce time) | [D] |
| `0x0a`, `0x0d`, `0x12` | 4 / 122 / 64 | Write only (GET fails). `0x0d` is the live LED frame | [I] |
| `0x10`, `0x13` | 16 / 10 | [?] | |

## Checksum **[C]**

Reports `0x06`, `0x07` and `0x08` end with a 16-bit little-endian checksum: the plain sum
of every preceding byte of the report (including report ID and size byte).

## Write handshake **[D]**

After every SET_FEATURE Swarm polls report `0x04`:

1. wait (Swarm uses 150 ms per poll)
2. GET_FEATURE `0x04`, look at byte 1
3. `1` = done, `2` = device rejected the data, `3` = busy, poll again (up to 100 times)

The same handshake after a control write works on the live device, where byte 1
returns `1`. **[C]**

## Reading a profile **[C]**

```
SET 0x04:  04 <profile 0..4> 80 00     then GET 0x06 (174 bytes)   settings
SET 0x04:  04 <profile 0..4> 90 00     then GET 0x07 (125 bytes)   buttons
```

Macros: `04 <profile> <slot> 00`-style control write, then GET `0x08` (1043 bytes) per
slot, 30 slots per profile. **[D]**, exact request byte not yet pinned down.

## Writing a profile **[D]**

Swarm sends the full report (`0x06` or `0x07`) with the profile index in byte 2 and a fresh
checksum, followed by the write handshake. There is no separate "commit" step in the code.
It only sends reports whose content changed.

Verified on the device for both reports (firmware 1.09): status goes busy (`3`) once, then OK (`1`), and the read-back matches. **[C]**

## Report 0x05 – profile info

```
05 04 <active profile> <profile count = 5>
```

Read: **[C]**. Writing the same 4 bytes with another index switches the active profile **[D]**.

openRoc reads this report once a second while its window is open, to notice a profile
switch made with a button on the mouse.

## Report 0x09 – device info

```
09 09 6d 66 0a 4e 00 00 00
      ^^ firmware version, 0x6d = 109 = "1.09"  [C]
```

## Report 0x06 – profile settings (174 bytes)

| Offset | Size | Field | |
|---|---|---|---|
| `0x00` | 1 | report ID `06` | [C] |
| `0x01` | 1 | size `ae` | [C] |
| `0x02` | 1 | profile index | [C] |
| `0x03` | 2 | `06 06` in every profile (sensitivity X/Y in older Kones) | [?] |
| `0x05` | 1 | enabled DPI stages, bit mask (`1f` = all five) | [I] |
| `0x06` | 1 | active DPI stage, 0-based | [I] |
| `0x07` | 5×2 | DPI stages, u16 LE, unit 50 DPI | [C] offset, [I] unit |
| `0x11` | 5×2 | second DPI table, factory values in all profiles (Y axis?) | [I] |
| `0x1b` | 1 | angle snapping, 0 = off, 1 = on | [D] |
| `0x1c` | 1 | `00` | [?] |
| `0x1d` | 1 | polling rate; 3 = 1000 Hz matches Swarm, 0–2 presumably 125/250/500 Hz | [D] |
| `0x1e` | 1 | lighting effect, see below | [C] |
| `0x1f` | 1 | effect speed, 1–11 in Swarm (factory `06`); 50 and 100 were accepted and ran about as fast as the top speed | [D], larger values [C] |
| `0x20` | 1 | brightness, `ff` = full | [D] |
| `0x21` | 1 | LED timeout in minutes (`0f` = 15) | [D] |
| `0x22` | 1 | energy-saving effect, 0 = off | [D] |
| `0x23` | 1 | set to 0 by Swarm whenever `0x21`/`0x22` change | [D] |
| `0x24` | 20×6 | LED entries: `<a> <b> R G B <c>`; factory reset writes `14 ff 00 48 ff`; positions below | RGB and positions [C], rest [?] |
| `0x9c` | 3 | `01 64 ff` | [?] |
| `0x9f` | 3 | profile colour, as shown on Swarm's profile slots | [C] |
| `0xa2` | 1 | `00` | [?] |
| `0xa3` | 1 | flag byte, cleared on lighting reset | [?] |
| `0xa4` | 1 | bit mask per DPI stage (Swarm's "AUTO" boxes?) | [D], meaning [I] |
| `0xa5` | 7 | zero | [?] |
| `0xac` | 2 | checksum | [C] |

Lighting effect numbers, each set on the device and observed **[C]**:

| Value | Effect |
|---|---|
| `01` | static colours |
| `02` | blinking |
| `03` | breathing |
| `04` | heartbeat |
| `05` | Photon FX (a pulse of changing colour running through the stripes) |
| `06`–`08` | accepted, look like static |
| `09` | wave (a rainbow moving across the mouse) |
| `0a` | AIMO intelligent lighting (factory default) |

`00` and values above `0a` have not been tried.

LED positions, found by lighting entries in distinct colours on the device **[C]**.
Each side of the mouse has four light stripes running from the cable end to the palm
end, with one LED at either end of every stripe. Entries are counted from 0, stripes
from left to right:

| Entry | Light |
|---|---|
| 0, 2, 4, 6 | left stripes 1–4, cable end |
| 1, 3, 5, 7 | left stripes 1–4, palm end |
| 8 | left fill light: shines into all four left stripes, faint while the stripe LEDs are on |
| 9 | right fill light, likewise |
| 10, 12, 14, 16 | right stripes 1–4, cable end |
| 11, 13, 15, 17 | right stripes 1–4, palm end |
| 18 | scroll wheel |
| 19 | button behind the wheel |

Swarm's eight colour zones are these groups: left front (0, 2, 4, 6), left rear
(1, 3, 5, 7), the two small front swatches (8 and 9), right front (10, 12, 14, 16),
right rear (11, 13, 15, 17), wheel (18) and button (19). The colours stored in profile 1
match the zone colours in a Swarm screenshot of the same profile. **[C]**

LED entry bytes seen so far: `a` = `01` (factory) or `14` (customised), `b` = `ff`,
`c` = `64` (100, likely brightness in percent).

Not in this report: debounce time and the distance control unit (lift-off). Both are
device-wide and live in their own reports, see the next section.

## Reports 0x0f and 0x11 – device-wide settings

These apply to all profiles. They are stored in the mouse, not in the Windows driver:
Swarm keeps both reports in its device object next to the profile data (`0x0f` as 6
bytes, `0x11` as 20 bytes) and its saved state for the wired Kone XP has no separate
debounce or DCU entry.

```
11 14 <debounce ms> 00 … 00 <checksum lo> <checksum hi>      live device: 11 14 09 … 2e 00
0f 06 <value> 00 60 34                                       live device: 0f 06 01 00 60 34
```

- `0x11` byte 2 is the value of the debounce control in Swarm's advanced settings; its
  reset button sets it to 10. The live device reads 9, and Swarm showed 9 ms. **[D]**
  The last two bytes fit the usual 16-bit sum (`11 + 14 + 09 = 2e`). **[I]**
- `0x0f` byte 2 belongs to the advanced-settings control listed just before the polling
  rate; its reset button sets it to 0. The live device reads 1 while Swarm showed the
  lift-off distance as "low", so this is probably the DCU setting. **[I]** The meaning of
  `60 34` is unknown; it is not a plain sum of the other bytes. **[?]**

Neither report has been written from openRoc yet. **[?]**

## Report 0x07 – profile buttons (125 bytes)

```
07 7d <profile>   30 × <4-byte slot>   <checksum lo> <checksum hi>
```

Slots 0–14 are the normal layer, slots 15–29 the Easy-Shift layer. **[I]**

Slot format **[I]**:

| Byte | Meaning |
|---|---|
| 0 | always `00` so far |
| 1 | keyboard: HID usage code of the key |
| 2 | keyboard: modifier mask (`02` Shift, `08` Cmd/GUI seen) – otherwise the function number |
| 3 | function group |

A keyboard slot stores a key *position* (HID usage), not a character. Usage `1c` types
"Y" under a US layout and "Z" under a German one, which is why Swarm on a German system
shows "Win+Z" for a slot holding `00 1c 08 06`. The GUI modifier (`08`) is the Windows
key on a PC and Cmd on a Mac.

Bytes 2 and 3 together form the function code Swarm uses internally
(`group << 8 | number`). The list below is taken from the menu-building code of
`KONE_XP.dll`; names marked ✓ were matched against what Swarm displays for bytes stored on
a real mouse, the others were assigned by their position in Swarm's menus.

| Code | Function | |
|---|---|---|
| `0000` | disabled | ✓ |
| `0101` `0102` `0103` | left click, right click (menu), middle click (universal scrolling) | ✓ |
| `0104` | double click | |
| `0105` `0106` | browser forward, browser back | ✓ |
| `0107` `0108` | tilt left, tilt right | ✓ |
| `0109` `010a` | scroll up, scroll down | ✓ |
| `0201` | cycle DPI | |
| `0202` `0203` | DPI up, DPI down | ✓ |
| `020c` `020d` | Easy-Aim / Easy-Wheel style functions with a parameter | |
| `0302` `0303` | previous track, next track | ✓ |
| `0304` | play / pause | ✓ |
| `0305` | stop | |
| `0306` `0307` `0308` | mute, volume up, volume down | ✓ |
| `030b`–`030f` | browser search, stop, reload, new tab, new window | |
| `0310`–`0312` | open preset folder / software via consumer keys | |
| `0403`–`0409` | single modifier keys; `0408` = left Shift ✓, others unmapped | |
| `06kk` | keyboard shortcut (see slot format) | ✓ |
| `0600` | macro (data in report `0x08`) | |
| `0801` | cycle profiles | ✓ |
| `0802` `0803` | next profile, previous profile | |
| `080b` | lighting on/off | ✓ |
| `09xx` | ROCCAT Talk functions | |
| `0a01` | Easy-Shift | ✓ |
| `0a02`–`0a05` | Easy-Shift lock and Talk variants | |
| `0bxx` | handled by Swarm on the PC (open application, document, folder, timers…) | |

Factory layout (profiles 2–4), slot → `fn/group`:

```
normal:      01/01 02/01 03/01 07/01 08/01 09/01 0a/01 02/02 03/02 05/01 06/01 key15 key0a 01/0a 01/08
Easy-Shift:  01/01 02/01 04/03 02/03 03/03 07/03 08/03 key22 key23 key20 key21 key1e key1f 00/00 0b/08
```

### Slot → physical input **[C]**

Tested on the device by binding a distinct key to every slot. The Easy-Shift layer uses
the same order, offset by 15.

| Slot | Input | Factory function |
|---|---|---|
| 0 | left button (not rebound in the test) | `01/01` click |
| 1 | right button (not rebound in the test) | `02/01` menu |
| 2 | wheel click | `03/01` middle click |
| 3 | wheel tilt left | `07/01` tilt left |
| 4 | wheel tilt right | `08/01` tilt right |
| 5 | wheel up | `09/01` scroll up |
| 6 | wheel down | `0a/01` scroll down |
| 7 | "+" button | `02/02` DPI up |
| 8 | "−" button | `03/02` DPI down |
| 9 | thumb T1 | `05/01` browser forward |
| 10 | thumb T2 | `06/01` browser back |
| 11 | thumb T3 | key `15` (R) |
| 12 | thumb T4 | key `0a` (G) |
| 13 | Easy-Shift button (by elimination) | `01/0a` Easy-Shift |
| 14 | button behind the wheel | `01/08` profile cycle |

Group `01` follows the numbering of older Kone models (1 click, 2 menu, 3 middle,
4 double click, 5 forward, 6 back, 7/8 tilt, 9/10 scroll). **[I]**

Verified by the same test: writing report `0x07`, key bindings with and without the
Shift modifier, and switching the active profile through report `0x05`. **[C]**

## Lighting effects over time

The mouse plays its effects itself; nothing about their timing is in the reports beyond
the effect number and the speed. There are two sources: Swarm's own simulation of the
effects in `aimo.dll` **[D]**, and a stopwatch held next to a mouse (firmware 1.09)
**[C]**. openRoc's preview takes the shape of each effect from Swarm and, where it was
timed, the pace from the mouse. `speed` is the stored speed byte, 1–11.

### Timed on the mouse **[C]**

Seconds for one full cycle, timed by hand, so good to a few tenths of a second:

| Effect | Timings (speed: seconds) | openRoc's preview |
|---|---|---|
| Blinking | 1: 2.0 (1 s on, 1 s off) · 10: 1.0 · 50: 1.0 | `2.11 − 0.11 × speed` |
| Breathing | 1: 8 · 5: 6 · 10: 3.5 · 50: 3.0 | `8.5 − 0.5 × speed` |
| Heartbeat | 1: 8 · 10: 4.5 · 100: 3.8 | `8.4 − 0.41 × speed` |
| Photon FX | 1: 8 · 6: 6.8 · 11: 5.1 | `8.37 − 0.29 × speed` |
| Wave | 11.5 at every speed | 11.5 |

The cycle time falls in a straight line with the speed; the breathing timing at speed 5
rules out a curve of the form `a + b / speed`. That speeds above 11 behave like 11 is
assumed from the timings at 50 and 100. **[I]**

- **Breathing:** between two breaths the mouse stays dark for 0.4–0.5 s, at every speed.
- **Heartbeat:** the shape matches Swarm's simulation.
- **Photon FX** (effect `05`): looks like Swarm's sequence, but runs in and out without
  a dark moment: when the pulse is back at the outermost stripe, violet, it turns round
  at once. A cycle is in and out again.
- **Wave** (effect `09`): starts with orange on the left and yellow on the right. The
  rainbow comes in from the right and moves left through green, turquoise, blue and
  pink. When the left side is pink and the right edge starts to turn red, it runs back
  the same way to orange and yellow. A cycle is there and back. The speed setting does
  not change it. openRoc's preview keeps the right edge one twelfth of the rainbow ahead
  of the left one; how the colours are spread in between was not looked at stripe by
  stripe. **[I]**

### Swarm's simulation **[D]**

Swarm's effect loop runs every 10 ms and calls each effect when its step time has
passed, so a step takes the next multiple of 10 ms. These step times do not match the
mouse: Swarm's heartbeat at speed 1 takes 23.5 s, the mouse 8 s. Swarm's wave also
differs from the mouse's: it runs in one direction only, spreads more of the rainbow
across the mouse and follows the speed setting. openRoc uses the table below only for
the shape of the heartbeat, the rainbow's colours and the frames of Photon FX.

| Effect | Step time (ms) | What a step does |
|---|---|---|
| Blinking | `100 / speed + 10` | on for 10 steps, off for 10 steps |
| Breathing | `150 / speed` | brightness rises linearly in 60 steps, falls in 60 |
| Heartbeat | `150 / speed + 10` | brightness changes by 1/50 per step: up to 100 %, down to 40 %, up to 75 %, down to 0, then 11 steps dark |
| Wave | 50, fixed | a rainbow of 384 colours (hue in steps of 4) moves by `2 × speed − 1` colours; neighbouring columns are 20 colours apart |
| Photon FX | `round((11 − speed) / 11 × 70 + 30)` | next frame of a fixed sequence of 151 frames, played forwards, then backwards, restarting at frame 12 |

Integer division throughout. Blinking, breathing and heartbeat dim the stored colours;
wave and Photon FX use their own.

For wave and Photon FX Swarm arranges the LEDs in eleven columns from left to right:
the four left stripes, the left fill light, wheel and button, the right fill light, the
four right stripes. Each stripe column holds two LEDs. **[D]** That the first of the two
is the cable end, as in report `0x06`, is assumed. **[I]**

Photon FX is a pulse that runs through the stripes from the outside in, on both sides
mirrored, visiting one stripe end after the other (up one stripe, down the next) and
changing colour from violet over blue, cyan, green, yellow and orange to red. A stop is
at full brightness for 13 frames and fades over about 19 frames before and after; the
eight stops reach full brightness at frames 25, 39, 61, 75, 97, 111, 133 and 147. Wheel
and button show the pulse's colour at 100/255, the fill lights stay off. openRoc
reproduces Swarm's frame table with a formula; the largest difference is a few
brightness steps.

Not read: which of Swarm's two breathing and heartbeat variants the Kone XP uses (the
plain ones are assumed), the direction of the wave, and AIMO. **[?]**

AIMO (`0a`) is the factory setting. openRoc shows it when a profile has it but does not
offer it for selection, because it belongs to ROCCAT's software running on the computer.

## Profile names **[I]**

The mouse does not store profile names. Neither profile report contains text, and the
names shown by Swarm (`LX-Mac`, `Default3`, …) are found only in Swarm's own files on the
PC (`SettingsServer_DLL.xml`, `ROCDevices.json`). openRoc therefore keeps names in a file
on the Mac and in its export files.

## Sources

- `roccat_swarm_monitor.exe` from ROCCAT Swarm 1.9388 (decompiled with Ghidra)
- Live reads from a Kone XP, firmware 1.09 – raw dumps in `backup/`
- `KONE_XP.dll` (Swarm 1.9473 device module) and Swarm's saved state from a Windows install,
  plus screenshots of Swarm showing the same profiles
- `aimo.dll` from the same Swarm install (decompiled with Ghidra) for how the lighting effects move
- [Eruption](https://github.com/eruption-project/eruption) `roccat_kone_xp.rs` for the lighting control reports
