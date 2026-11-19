# Draughts

A local, two-player English draughts game built with Rust and WASM Runner. Open
the board in one browser and take turns on the same device. Charcoal goes first.

## Build and play

With a current `wr` installed, run from the repository root:

```sh
wr update draughts # first build or after changing dependencies/functions
wr build draughts
wr --state-dir "$PWD/draughts/.wr-data" daemon start --listen 127.0.0.1:9000
wr --state-dir "$PWD/draughts/.wr-data" deploy draughts
```

Open **http://127.0.0.1:9000/games/draughts/**. Select an outlined piece, then a
gold destination dot. The board supports keyboard navigation with Tab and
Enter/Space. Flip the board whenever useful. New game asks before replacing
the current position.

The dedicated daemon uses the installed `wr` version and avoids conflicts with
other applications or older running daemons. To stop it:

```sh
wr --state-dir "$PWD/draughts/.wr-data" daemon stop
```

After edits, rebuild and deploy again. After restarting the daemon, deploy
again; deployments are in memory, while SQL data persists in the state directory.
Always use the same state-directory path to retain saved games. Do not delete
`.wr-data` unless you intend to discard them.

## Rules

- Standard 8 × 8 English draughts, 12 pieces per player.
- Men move and capture diagonally forwards; kings move and capture both ways.
- Captures are compulsory. If several are available, the player may choose any.
- Continue a capture chain with the same piece until no more jumps are available.
- Reaching the far rank crowns a king and ends the turn, even during a capture.
- A player wins when the opponent has no pieces or no legal moves.
- Draws are by agreement; repetition and move-count draws are not automated.

## Saved games and API

The browser stores only a random game ID in local storage. The authoritative
board, player turn, pending capture, winner, last move, and completed-turn count
are stored as JSON in the `games` SQL table. The schema is installed by
[the SQL migration](migrations/0001-games.apply.sql), through `wr`'s SQL support.
There is no in-memory or browser-only game-state fallback.

Refreshes, tabs on the same origin, and later visits in the same browser share
the saved game. Different browsers or origins have separate games. Clearing
browser storage loses the pointer to the old game. This is a local game, not an
authenticated multiplayer service; the ID identifies the game and is not an account.

Both endpoints below require `X-Game-Id`, a 32-character hexadecimal identifier:

| Method and path | Behaviour |
| --- | --- |
| `GET /games/draughts/api/game` | Create or load the browser's game; return game, legal moves, and revision. |
| `POST /games/draughts/api/game` | Apply `{"action":"move","revision":0,"from":40,"to":33}` or `{"action":"reset","revision":0}`. |

Squares use row-major indices from 0 (`a8`) to 63 (`h1`). Positive pieces are
charcoal, negative pieces ivory; magnitude 1 is a man and 2 is a king. The server
validates every move. SQL compare-and-swap updates reject stale revisions with
409; the browser then reloads the latest board. Invalid input returns 400 and
illegal moves return 422 without changing state. Reset also requires a current
revision, protecting a newer game from stale tabs.

## Development and verification

- Routes and dependencies: [application.yaml](application.yaml).
- HTTP handlers: [functions/](functions/); shared game rules and SQL access:
  [libs/](libs/).
- Self-contained, responsive UI: [templates/index.html](templates/index.html).
- `wr` manages the generated Cargo workspace and `.wr/` outputs. Declare
  dependencies in the application manifest, not generated Cargo files.
- Commit `Cargo.lock`; only run `wr update` when intentionally refreshing it.
- The parent workspace already links the application for rust-analyzer.

After `wr build draughts`, run the rule tests (the shared module is identical in
all generated handler crates, so one crate is sufficient):

```sh
cargo test --manifest-path draughts/Cargo.toml -p wr-function-play --locked
python3 draughts/tests/smoke.py
```

The smoke test needs the deployed daemon. It creates isolated test game IDs and
checks load/save, session separation, malformed/illegal commands, stale moves,
concurrent writes, and reset. Pass `--base-url` to test a different local origin.

The workspace **Build draughts** task builds the application. **Deploy draughts**
builds, starts the dedicated daemon if necessary, and deploys the current build.
