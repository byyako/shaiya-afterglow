# Custom animations

This page describes how a character emote animation is stored, selected and played in the US client (build 2010-04-26, patch 108), what
a replacement has to preserve, how a candidate is validated and staged in the archive, and what was actually seen in game. It is based on
an offline set of tools (described in `tools-and-method.md`, not offered) that read and write `.ANI` files byte for byte and preview them.
The format itself is in `formats.md`; the playback math is in its animation section. Status words: **confirmed** (read in the binary or
reproduced by a test), *inferred*, **live** (seen in a game session).

## In short

- **A custom emote is an `.ANI` file with the same skeleton as the one it replaces.** Bone count, order, parents and the reference matrices
  must stay byte-identical; only the rotation and translation keys change.
- **Each of the 16 character rigs is its own target.** A file authored for one rig cannot be reused on another, and even the ten emotes of
  one rig carry different reference data.
- **The tested route replaces an existing emote** (the victory emote, ALT slot 117), so no server or command change is needed. A separate new
  command is a larger feature.
- **What is proven in game:** basic playback of a replaced victory emote on the human male Fighter rig. Equipment, interruptions, two-client
  views and the other 14 rigs are untested.
- **The first attempt failed because race and sex do not pick the rig:** replacing only the `humm` file left the Fighter's stock victory;
  the Fighter uses `huwm`.

### Using this page

- **If you make an emote:** read "Format contract", then "Authoring and validation"; the client repository's animation tools check a candidate against the rig it replaces and report any difference.
- **If you patch or mod the client:** "Staging in the archive and rollback" says how to put the file in and take it out again; the [archive](glossary.md#archive) is in [data-archive.md](data-archive.md).
- **To check a claim yourself:** the `.ANI` layout and the playing code are in [formats.md](formats.md); use [tools-and-method.md](tools-and-method.md) for the tools.

## Format contract (confirmed)

An `.ANI` is headerless and little-endian:

```
i32 first_header_field       preserve from the chosen reference
i32 end_key                  key-time endpoint, not a render-frame count
u16 bone_count
repeat bone_count:
    i32 parent               -1 root; otherwise an earlier zero-based index
    f32 reference_matrix[16] preserve all 64 original bytes
    i32 rotation_key_count;    repeat: i32 key, f32 quaternion[4] (x, y, z, w)
    i32 translation_key_count; repeat: i32 key, f32 translation[3]
```

There is no magic header, no bone names, no scale keys and no other quaternion order. Keep the target's helper, clothing, hair and
attachment bones: deleting "unused" bones changes the indexed contract with the meshes. Playback uses **30 key units per second**: an
eight-second clip has end key 240 and keys 0, 2, 4, …, 240; a 60 FPS renderer does not make it shorter.

**Pose and skinning math** use D3DX row vectors, with translation at flat matrix indices 12–14:

```
world[i]        = local[i] * world[parent[i]]
local[i]        = world[i] * inverse(world[parent[i]])
mesh rest world = inverse(mesh_inverse_bind[i])
skin transform  = mesh_inverse_bind[i] * animated_world[i]
```

A vertex blends the two skin matrices of its two bones with its weight (the second weight is `1 − first`). The ANI reference matrix is
not the mesh's inverse bind matrix; the ANI's bind pose and the mesh's differ. The mapped examples use Y up and −Z forward; confirm the axes
for any other rig before converting. Convert complete transforms consistently; swapping quaternion components alone is not enough.

**ALT playback metadata.** After the NUL-terminated ANI name, a populated `AL3` slot has 24 bytes: `u8 mode`, 3 padding bytes, an `i32` follow-up
field and four `f32` event keys. The padding can contain uninitialised garbage (five emote records have non-zero padding), so the mode must
be read as a byte. All 160 stock emotes have mode 2 and follow-up 0. An ANI-only replacement leaves the ALT unchanged.

## The 16 rigs

The action list is chosen by `0x53f230` from three object fields: race at `+0xc`, then zero/nonzero branches at `+0xd` and `+0xe`, recorded
here as race/style/variant. Every list has 126 slots, with emotes at 116–125. The tested replacement slot is **117**, selected by the
outgoing motion `0x74` (116) of `/victory`; the motion id and the ALT slot are different numbers (the pair is adjacent, 116↔117).

| Race folder | Rigs (selector race/style/variant, bones) |
|---|---|
| Human | `humf` 0/0/0 (38), `humm` 0/0/1 (56), `huwf` 0/1/0 (41), `huwm` 0/1/1 (65) |
| Elf | `elmr` 1/0/0 (50), `elmm` 1/0/1 (58), `elwr` 1/1/0 (60), `elwm` 1/1/1 (61) |
| Vile | `vimr` 2/0/0 (36), `vimm` 2/0/1 (51), `viwr` 2/1/0 (44), `viwm` 2/1/1 (55) |
| DeathEater | `demf` 3/0/0 (44), `demr` 3/0/1 (42), `dewf` 3/1/0 (51), `dewr` 3/1/1 (55) |

Totals: 16 action lists, 160 stock emote slots, 96 representative mesh parts, 807 indexed reference bones. Bone indices are zero-based;
bind positions are the torso mesh's inverse-bind translation and help identify chains but are not anatomical names. **All nine other emotes
in a rig have a different full reference fingerprint from slot 117**, so a file authored for slot 117 is not an acceptable replacement
for slot 118. Per-rig bone lists are a data extract from the game's models and are not published.

## Authoring and validation

A candidate is derived from the exact target `.ANI`: export it to editable JSON (`shaiya-ep4-ani-edit-v1`: header fields, and per bone the
index, parent, matrix, rotation keys and translation keys), change only track values and end time, and import it back against the same
reference, which checks the skeleton. For a mapped rig a procedural author builds a choreography from an explicit bone map (56 bones for
`humm`, 65 for `huwm`: the spine, head, two arm chains and two leg chains with foot IK that keeps soles level and preserves segment
lengths); the other 14 rigs still need an anatomical map before anything can be authored.

Every candidate must pass:

- the strict validator without errors (warnings reviewed; a zero exit status alone is not enough);
- the same bone count, order, parents and reference bytes as the target;
- binary → JSON → binary byte-identical;
- tracks that start at zero, keys strictly increasing and at least two apart, ending with the header, finite `float32` values;
- quaternions `x, y, z, w` of unit length within 0.001, adjacent signs continuous;
- evaluation of the **serialised** output at every integer client key with all preview meshes: finite poses and vertices, a fixed world root
  for an in-place emote, preserved non-pelvis bone lengths, no discontinuities or implausible mesh excursions;
- first and last poses compared for the intended return behaviour;
- a visual check of arms, knees, hands, feet, head, clothing and attachments from several angles, because a plausible render can hide wrong
  indices in a mesh.

The archive-wide round trip of the stock emotes passes for all 160; warnings appear in 17 stock files (stock observations, not permission to
copy the quirks), and 20 monster and NPC `.ANI` files hold non-finite matrices, so a character-emote pass is not a clean whole-archive pass.

## Staging in the archive and rollback

The target path pattern is `Character/<race folder>/ANI/<rig>_117_emoti002.ANI`; a loose renamed file beside the executable has no verified
load path. A replacement is staged in a fresh folder with exactly one file, an archive pair is built from the current one with the
repacker (`data-archive.md`) or by appending the candidate to the end of `data.saf` and changing only that index entry, and the result is
checked: identical path sets, every unchanged asset's content identical (not offsets), no duplicates, no overlaps, bounds correct. The
whole matched `data.sah`/`data.saf` pair is backed up first, the two files are swapped while no client can read them (a pair of renames is
not crash-atomic), both installed hashes are verified and the target is extracted, hashed and parsed, then the exact intended executable is
restarted (animation caches and archive handles retain old data until then). Rollback restores the pair together, hashes it, restarts and
verifies stock behaviour.

## What was seen in game

| Check | Result |
|---|---|
| Replaced victory emote on the human male Fighter rig (`huwm`), played with `/victory` | basic playback reported working (**live**) |
| Same replacement on `humm` only | the Fighter still played the stock emote: **race and sex alone do not select the rig** |
| Everything else (equipment and attachments, interruptions and transitions, zoning and reconnect, a second client, the other 14 rigs) | not tested |

The packet log shows **`0x0506` for both the request and the actor-motion receive**, with outgoing motion byte `0x74`; **`0x0507` is
equipment replication in this client**, not the emote broadcast, and animation frames are local assets that travel in no packet. Reusing the
existing command needed no server change. A stock client sees its own stock animation for the reused motion id. A separate `/dance` that
keeps `/victory` is a distinct feature: command parsing, one-byte motion ids, receive mapping, slot counts, per-rig ALT selection,
completion and interruption rules, and server validation would all need an audit.

Tested pieces: lossless binary and JSON round trips, malformed-input tests, an offline animated HTML preview with a timeline, skeleton
overlay and before/after comparison, and a 33-test regression suite. No validated Blender or other DCC bridge exists: OBJ output is a posed
static mesh, not an armature, and any bridge would have to preserve bone indices, parents, weights, inverse binds, row/column conventions,
quaternion order and helper bones, prove a stock round trip numerically and in game, and bake evaluated local transforms after constraints
or IK.
