# Team Numbers

Team numbers appear in the `team_num` column of `Demo.players` and correspond to
the `m_iTeamNum` field on entity classes.

| team_num | Team Name | Description |
|----------|-----------|-------------|
| 0 | Unassigned | No team assigned |
| 1 | Spectator | Spectating the match |
| 2 | Hidden King | One of the two competing teams (typically shown at the bottom of the map) |
| 3 | Archmother | One of the two competing teams (typically shown at the top of the map) |

## Lane Assignments

The `start_lane` column in `Demo.players` indicates each player's original lane
assignment at the start of the match. Lanes are relative to the team's side of the
map.

| start_lane | Lane |
|------------|------|
| 1 | York (Yellow) |
| 4 | Broadway (Blue) |
| 6 | Park (Green) |
