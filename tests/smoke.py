"""HTTP integration checks against a running draughts app; no third-party packages."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from uuid import uuid4


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:9000/games/draughts")
    args = parser.parse_args()
    endpoint = args.base_url.rstrip("/") + "/api/game"
    game_id = uuid4().hex

    def call(command=None, session=game_id, raw=None):
        headers = {"X-Game-Id": session, "Content-Type": "application/json"}
        body = raw if raw is not None else json.dumps(command).encode() if command is not None else None
        request = Request(endpoint, data=body, headers=headers)
        try:
            response = urlopen(request, timeout=10)
        except HTTPError as error:
            response = error
        with response:
            return response.status, json.load(response)

    status, initial = call()
    assert status == 200 and initial["revision"] == 0, (status, initial)
    assert len(initial["legal_moves"]) == 7
    assert initial["game"]["board"].count(1) == 12
    assert initial["game"]["board"].count(-1) == 12
    assert call(session="")[0] == 400
    assert call(raw=b"not-json")[0] == 400
    assert call({"action": "move", "revision": 0, "from": 40, "to": 0})[0] == 422
    assert call({"action": "move", "revision": 0, "from": 1000, "to": 33})[0] == 422
    assert call()[1] == initial, "Invalid moves changed the state"

    status, moved = call({"action": "move", "revision": 0, "from": 40, "to": 33})
    assert status == 200 and moved["revision"] == 1
    assert moved["game"]["turn"] == -1 and moved["game"]["board"][33] == 1
    assert call()[1] == moved, "Saved game did not survive a new request"
    assert call(session=uuid4().hex)[1] == initial, "Separate browser games leaked state"
    assert call({"action": "reset", "revision": 0})[0] == 409
    assert call({"action": "move", "revision": 0, "from": 17, "to": 24})[0] == 409
    assert call()[1] == moved, "A stale request changed the state"

    # Two clients play against the same revision. Exactly one write may commit.
    commands = [dict(action="move", revision=1, **{k: m[k] for k in ("from", "to")})
                for m in moved["legal_moves"][:2]]
    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(call, commands))
    assert sorted(status for status, _ in results) == [200, 409], results
    status, saved = call()
    assert status == 200 and saved["revision"] == 2 and saved["game"]["moves"] == 2
    status, reset = call({"action": "reset", "revision": 2})
    assert status == 200 and reset["revision"] == 3
    assert reset["game"] == initial["game"] and reset["legal_moves"] == initial["legal_moves"]
    assert call()[1] == reset
    print("PASS: initial board, validation, SQL save/load, session isolation, stale revisions, concurrent writes, reset")


if __name__ == "__main__":
    main()
