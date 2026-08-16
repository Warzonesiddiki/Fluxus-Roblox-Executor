# Apex UNC (AUNC) — Script API Specification (Blueprint B1.3)

**Version:** 0.1 · **Status:** Draft for review
**Scope:** The complete script API surface for Pillar C (control scripts) and Pillar B (in-game execution), defined so two developers could implement it independently and interoperate.

---

## 1. Design principles

1. **Two surfaces, one spec:** `Control API` (Pillar C — drives automation from sandbox) and `InGame API` (Pillar B — Synapse-X-compatible surface for in-process execution on our server).
2. **Deterministic semantics:** every function has one defined behavior; no unspecified edge cases.
3. **Unsafe-free by default:** Control API has no raw memory access, ever.
4. **Versioned:** the spec version is exposed as `_AUNC_VERSION` in every runtime.
5. **Testable:** each section maps to conformance tests (see §7).

---

## 2. Control API (Pillar C — sandboxed automation scripts)

### 2.1 Execution model

- Scripts run in the wsx-core sandbox (memory limit, instruction limit, timeout).
- The runtime calls script-registered callbacks on game events; the script calls API functions to act.
- A script registers callbacks by assigning globals:

```lua
-- example control script
function on_tick(state)            -- called every tick (20 Hz)
    if state.backpack_fill > 0.95 then
        move_to_hive()
    end
end

function on_quest(quest)           -- called when a quest is detected
    collect(quest.target_field)
end
```

### 2.2 Callbacks (script → engine)

| Callback | Args | Semantics |
|---|---|---|
| `on_tick(state)` | `state: GameSnapshotTable` | Called every 50ms tick with current fused state. |
| `on_quest(quest)` | `quest: {id, title, target_field, target_amount}` | Fires when the quest planner detects a new/updated quest. |
| `on_backpack_full(fill)` | `fill: number (0..1)` | Fires when fused backpack fill crosses ≥0.95. |
| `on_death()` | — | Fires when respawn overlay detected. |
| `on_connect(client)` | `client: {id, name}` | Fires when a new game client is detected. |

### 2.3 Actions (script → engine)

| Function | Signature | Returns | Notes |
|---|---|---|---|
| `move_to(x, y)` | `(number x, number y)` | `boolean` | Humanized move to normalized position (0..1). Returns false if path blocked. |
| `move_to_field(name)` | `(string name)` | `boolean` | Route to named field via macro routes. |
| `move_to_hive()` | `()` | `boolean` | Route to hive + convert. |
| `collect()` | `()` | `boolean` | Trigger gather at current position. |
| `collect_tokens(filter)` | `(table? filter)` | `number` | Collect visible tokens, optionally filtered by type; returns count. |
| `press(key)` | `(string key)` | `boolean` | Humanized key press (e.g. "e", "r", "w"). |
| `hold(key, ms)` | `(string key, number ms)` | `boolean` | Humanized hold. |
| `stop()` | `()` | `boolean` | Stop all automation until next tick. |
| `wait_for(condition_fn, timeout_s)` | `(function, number)` | `boolean` | Poll condition until true or timeout. |

### 2.4 State queries (script → engine)

| Function | Returns | Notes |
|---|---|---|
| `pollen()` | `number` | Fused backpack fill 0..1. |
| `position()` | `(number, number)` | Fused normalized position. |
| `quest()` | `table?` | Current quest info or nil. |
| `field()` | `string` | Current field name. |
| `is_dead()` | `boolean` | Respawn overlay present? |
| `tokens()` | `table` | Array of `{x, y, type, ttl}` visible tokens. |
| `vision_state()` | `table` | Raw ScreenState (debug). |
| `net_state()` | `table` | Raw EntitySyncState (debug). |

### 2.5 Logging

| Function | Notes |
|---|---|
| `print(...)` | Streams to console panel (sandbox output buffer). |
| `warn(...)` / `error(...)` | Same, tagged. |

---

## 3. InGame API (Pillar B — Synapse-X-compatible, own server only)

### 3.1 Execution model

- Loaded into the game client process on our server via wsx-core injector.
- Provides a `loadstring`-equivalent + full Luau environment access.
- **Policy gate:** runtime refuses to initialize unless the target matches the allowlist.

### 3.2 Core globals

| Global | Type | Notes |
|---|---|---|
| `game` | Instance | Root data model (Read/Write, as in Synapse X). |
| `workspace` | Instance | Alias for `game.Workspace`. |
| `getgenv()` | table | Per-executor persistent environment. |
| `getreg()` | table | Registry of globals. |
| `getgc()` | table | Garbage-collector objects. |
| `getrawmetatable(t)` | table | Raw metatable access. |
| `setreadonly(t, bool)` | — | Toggle table readonly. |
| `isreadonly(t)` | boolean | Query. |
| `loadstring(src)` | function | Compile+return chunk (synapse semantics). |
| `hookfunction(target, hook)` | (function, function) | Function hooking (synapse semantics). |
| `hookmetamethod(obj, method, hook)` | function | Metamethod hooking. |
| `getnamecallmethod()` | (string, table) | Namecall introspection. |
| `Drawing` | table | 2D drawing library (Line, Square, Circle, Text, Image, Quad, Triangle). |
| `fireclickdetector(detector)` | — | Fire a ClickDetector. |
| `request({url, method, headers, body})` | table | HTTP request (gated; fails closed on prod). |
| `readfile(path)` / `writefile(path, data)` | — | Sandboxed file access under `apex/` dir. |
| `isfile(path)` / `delfile(path)` | boolean / — | File utils. |
| `getfpscap()` / `setfpscap(n)` | number / — | FPS controls. |
| `getconnections(signal)` | table | Signal introspection. |
| `queue_on_teleport(code)` | — | Run code after teleport. |
| `setclipboard(text)` | — | Clipboard write. |
| `crypt` | table | Base64, hash, random (synapse-compatible). |
| `debug` | table | `getreg`, `getinfo`, `traceback` (as provided by Synapse; NOT raw Lua debug). |

### 3.3 UNC compatibility statement

The InGame API targets **100% UNC + sUNC compliance**. Conformance suite §7 runs the community UNC test battery against the runtime on our server; the spec pins required scores:
- UNC ≥ 95% (target 100%)
- sUNC ≥ 90% (target 100%)

---

## 4. Data structures (shared)

### GameSnapshotTable (serialized to Lua table)

```lua
{
  backpack_fill = 0.42,          -- number 0..1
  honey = "125,430",             -- string (OCR)
  is_dead = false,               -- boolean
  quest_ready = false,           -- boolean
  quest = {                      -- nil or table
    id = "q123", title = "...", target_field = "Pine Tree Forest"
  },
  field = "Pine Tree Forest",    -- string
  position = { x = 0.62, y = 0.48 },  -- normalized
  tokens = { { x = 0.5, y = 0.6, type = "Ability", ttl = -1 } },
  confidence = 0.93,             -- fusion confidence 0..1
}
```

---

## 5. Error semantics

- All Control API actions return `false` + set `_AUNC_LAST_ERROR` (string) on failure.
- Engine-level errors (sandbox limit, policy gate) raise a Lua error with a stable error code prefix: `AUNC-E<code>`:
  - `E1` sandbox memory limit · `E2` instruction limit · `E3` timeout · `E4` policy gate · `E5` bad argument · `E6` target not reachable

---

## 6. Security rules

1. Control API: no file/network/env access (except gated `request` in InGame API).
2. InGame API file functions restricted to `apex/` sandbox dir.
3. `request` in InGame API: gated to allowlist domains; hard-fails against production targets.
4. Pillar B runtime refuses non-allowlisted targets (policy gate, fails closed).

---

## 7. Conformance test suite (AUNC-T)

Each spec section has conformance tests run in CI (and against our server for Pillar B):

| Suite | Covers | Pass bar |
|---|---|---|
| `control_api_basic` | every §2 function exists, has correct arity | 100% |
| `control_api_semantics` | behavior on empty args, nil state, full backpack | 100% |
| `control_script_sample` | the §2.1 example script runs and drives actions | 100% |
| `ingame_unc_core` | UNC core battery (game, getgenv, loadstring, ...) | ≥95% |
| `ingame_sunc_advanced` | sUNC battery (hooks, drawing, namecall) | ≥90% |
| `ingame_drawing` | all 8 Drawing primitives create/destroy | 100% |
| `policy_gate` | Pillar B refuses non-allowlisted target | 100% (must fail) |

---

## 8. Open questions

- Q1: Should Control API include `loop(fn)` for engine-managed loops, or is `on_tick` sufficient? **Default: on_tick only.**
- Q2: InGame API `debug` table — full Synapse surface or minimal? **Default: Synapse-compatible.**
- Q3: `request` allowlist — config file or compile-time? **Default: compile-time allowlist + config override (own server only).**

---

*Next: implement AUNC runtime skeleton in wsx-core (M2.4), conformance suite scaffolding.*
