# Team Numbers

`team_num` uses the recorded entity field `m_iTeamNum`.

| Value | Name |
| --- | --- |
| 0 | Unassigned |
| 1 | Spectator |
| 2 | Hidden King |
| 3 | Archmother |

## Lane Assignments

`start_lane` records the player's original lane assignment.
The protobuf `CMsgLaneColor` defines these values:

| Value | Color |
| --- | --- |
| 0 | None |
| 1 | Yellow |
| 3 | Green |
| 4 | Blue |
| 6 | Purple |

Map names and layouts can change. Do not infer a street name from the color alone.
