# Visual Design

Use this guide for Boon plots and other Deadlock views.
Plot styles are private implementation details, not a public API.
Keep item IDs, map bounds, and game calculation rules in replay data or boon-data.

## Colors

Use sRGB hex colors in `#RRGGBB` format.

| Role | Hex | Use |
| --- | --- | --- |
| Background | `#101720` | Figure background |
| Surface | `#18232f` | Table rows |
| Text | `#e8edf2` | Main values and labels |
| Muted text | `#9caabe` | Secondary labels and unknown values |
| Axis | `#354251` | Axis borders |
| Team 2: gold | `#dfb85b` | Team ownership |
| Team 3: blue | `#7ab6e9` | Team ownership |
| York: yellow | `#fbbf24` | Lane-name badge |
| Broadway: blue | `#60a5fa` | Lane-name badge |
| Greenwich: green | `#34d399` | Lane-name badge |
| Weapon: orange | `#df9b46` | Weapon item borders |
| Vitality: green | `#77b255` | Vitality item borders |
| Spirit: purple | `#c27ce0` | Spirit item borders |
| Gray | `#9caabe` | Unknown item types and neutral teams |
| Health | `#8ed29b` | Recorded health bars |
| Health track | `#334050` | Health-bar background |

Item categories come from `m_eItemSlotType` in the selected catalog.
They do not change team colors. Name teams in section headings and legends.
Use shapes with color to show dead players and destroyed objectives.
Current primary lane IDs
are `1` (York), `4` (Broadway), and `6` (Greenwich). Unknown lanes use gray.

## Text and Values

Use DejaVu Sans. Use 17-point titles, 11-point section headings, 10-point body
text, 9-point column headings, and 8-point captions. Use Title Case for headings.
Keep player names and custom titles as supplied.
Center numeric table cells. Put units in labels or captions.

Use `k` for metric values in the thousands, with at most one decimal place.
Keep IDs and ticks in full.
Remove an unused decimal zero. Use whole numbers below 1,000.
A zero stays `0`. A missing total is `—`.
Label an unknown objective value `Unknown`.

Use match-clock minutes on timelines. Label the axis `Time Since Start`.
Use `mm:ss` for match duration and replay time.
State the units when you add another metric.

## Layout

Center the summary title and subtitle. Keep table headings close to the rows.
Center hero portraits, names, values, and item grids on each player row.
Use two item rows: five columns for up to ten items, six for 11–12.
Keep extra items visible if a larger build needs more rows.
Mark the winning team with `*`. Show the winner and duration in the subtitle.

Left-align timeline and position titles. Use the same title size as the summary.
Keep legends and labels clear of data. Reduce empty space at the plot edges.

## Map Symbols

| Symbol | Meaning |
| --- | --- |
| Circle or hero icon | Player position |
| Dim cross | Dead player |
| Square | Objective |
| Dim cross with an objective label | Destroyed objective |
| Small dot | Trooper |
| Team-colored lines with a thin dark outline | Zipline ownership |
| Colored name badge | Zipline lane |
| Small perpendicular tick | Displayed control boundary |
| Bar below a player | Recorded health |
| Line from a player | Viewing direction, when selected |

Draw the map, ziplines, troopers, objectives, players, and labels in that order.
Use Source world coordinates with +Y up. Read map bounds from the image catalog.
A map layer selects an image. Use a separate altitude filter to select a floor.
For last-seen views, use a distinct marker. Name the observing team.

## Data Rules

Use Steam IDs to join player records. Use the hero ID from the selected record.
Do not replace missing coordinates with zero.
Leave gaps when a team total has missing values or players.
Use the last recorded objective state at or before the selected tick.
Keep its limits clear when removal data is absent.

Summary totals come from the post-match message.
Position plots use exact-tick snapshots.
If you add plots for calculated stats, state the data version, mode, and ruleset.
Identify partial results and keep their diagnostics available.

## Units

- Colors use sRGB hex values.
- Text, icons, borders, line widths, bars, and label offsets use points.
- Scatter marker sizes use square points.
- Figure width and height use inches.
- Summary row and column positions use internal table units.

## Export and Changes

Apply styles to the plot's figure and axes. Keep global matplotlib settings unchanged.
All plot functions return `(fig, ax)` and accept `ax=`.
Set export size and resolution for the target medium. Keep labels inside the image.
Use PNG for raster images or SVG for vector output. Icons remain raster images.

After a style change, check the summary, timeline, and position reference plots.
Include long names, missing values, dense map markers, and twelve-item builds.
Close figures after use.
