# Client file formats

This page specifies the file formats inside the US client's `data.saf` archive (build 2010-04-26, patch 108): models, animations,
effects, maps and the model lists. Most layouts were first published by Castor (castor4878) on elitepvpers; they are restated here
and **checked against every file of that type in the 108 archive with a parser** (the client repository's `tools/formats.py check`; a file passes only if the
parser consumes it to the exact last byte). The loader functions named below are in `game.exe`.

## In short

- **Models** are plain binary files: static meshes (`.SMOD`, `.3DO`, `.3DE`, `.VANI`), skinned meshes (`.3DC`) with skeleton
  animations (`.ANI`), and model lists (`.MLT`, `.ITM`, `.MON`) that item and monster definitions point into by record index.
- **Effects** (`.EFT`) are particle emitters and sequences; **maps** are `.WLD` (fields and dungeons) plus `.DG` dungeon geometry.
- **Every file of each type in the archive parses to the last byte** with the layouts below (counts per format), so a tool built on
  them reads the whole game.
- **Animation is plain math:** quaternion keys per bone, D3DX row-vector matrices and a two-bone blend. The section on playing `.ANI`
  on `.3DC` is enough to export working models.
- **Only SData tables are encrypted;** models and maps are plain.

Conventions: all values little-endian; `i32`/`u16`/`f32` etc.; `str` = `i32 len` + `len` bytes (usually NUL-terminated, cp949);
`face` = `u16 i1, i2, i3` (a triangle of vertex indices; every Shaiya mesh is triangles). Status words: **confirmed** (the parser
consumes every file; loader address given), *inferred*.

Sources (elitepvpers, Shaiya PServer sections): the "Shaiya File Formats" sticky (thread 4462055, Castor's posts 2020-01..04),
"Question about SMOD file format" (4638178, 2019), "Trying to read Bone info in 3DC files" (4664506, 2019), "DataSah" (3621923),
"Data tool crash on specific offsets" (3441524). The struct text built into Castor's shStudio agrees (see `tools-and-method.md`).

## Static meshes

### .SMOD — building/object "super module" (Entity/…)
```
f32 center[3]
i32 unk                 always 0 in files checked (maybe a str length, or a glow value)
f32 min[3], max[3]      bounding box
i32 nModules
  Module: str texture (".tga" name; the client uses the .dds), i32 nVert, Vert[nVert], i32 nFace, face[nFace]
     Vert: f32 pos[3], f32 normal[3], u32 color (0xFFFFFFFF; "unk"), f32 u, v      (36 bytes)
f32 box2min[3], box2max[3]   inside min/max, not always equal (purpose unknown)
i32 nExtra              0 in almost all files
  Extra: i32 nPoint, f32[3]*nPoint, i32 nFace, face[nFace]   (untextured; likely collision)
```
Loader: `SModel::Load` `FUN_00598dc0`. The first 16 bytes are a bounding sphere (centre, radius; radius is always 0 in the files)
that the client **recomputes** from min/max (centre of the box, half its diagonal, at least 2.0). `nExtra` is a **flag**, not a count:
if non-zero, one collision mesh follows (`nPoint, f32[3]*nPoint, nFace, face[]`). **confirmed: 1355/1355** (the one broken file,
`R1_Trade.SMOD`, was removed in the cleanup).

### .3DO — weapon/item mesh
Loader `FUN_005437a0` (also used for `Data/Item/3DO/arrow.3DO`, a literal in `FUN_0040d880`).
```
str texture; i32 nVert; {f32 pos[3], f32 normal[3], f32 u, v}[nVert]; i32 nFace; face[nFace]
i32 frameRate; i32 nFrames                  (0, 0 in every file except arrow.3DO: 20, 11)
  Frame: i32 key; {f32 pos[3], f32 normal[3]}[nVert]       vertex animation (the flying arrow)
```
No bones: vertices are relative to the attach point of the hand. Texture name is usually empty (the `.ITM` record picks the DDS).
**confirmed: 271/271.**

### .3DE — effect mesh with per-vertex animation
Loader `FUN_00544a10` (via `FUN_00538af0`, which caches by name; every 3DE an `.EFT` lists is loaded with the EFT).
```
str texture (the published "signature 0" is just an empty string; about half the files name a .dds); i32 nVert;
{f32 pos[3], i32 bone (-1), f32 u, v}[nVert]; i32 nFace; face[]
i32 maxKey; i32 nFrames
  Frame: i32 key, {f32 d[3], f32 u, v}[nVert]     (movement and UV shift per step; keys ascend, not always even)
```
`nFrames == 0` (177 files): the client builds one frame from the base vertices. The client checks nothing: it reads the layout
above and keeps whatever it gets. **confirmed: 469/469** (all files in the original archive too, except the 0-byte `test1.3de`),
466 in the layout above. The others do not follow it, and the client misreads them:

| File | What it really holds | What the client does |
|---|---|---|
| `004.3de` (listed by 28 `.EFT`) | 31 texture names, then a 3785-vertex mesh, 0 frames | reads the 2nd name's length as nVert: garbage mesh; but no emitter of those EFTs uses it, so it is never drawn |
| `undead5-02.3de` (removed) | the texture name twice, then a normal mesh | same misread |
| `Deathknight007.3DE` (2 emitters) | frame vertices of 16 bytes (`pos[3]`, one f32) | reads 20-byte frame vertices: frames 2–5 garbled, the read runs past the end |
| `d_dragonstone03.3de` (1 emitter) | ends after the faces | maxKey/nFrames stay 0 (zeroed object): a static mesh, as intended |
| `angel.3de`, `waterrising00.3de` (removed) | 3 and 4 complete meshes concatenated | reads the first one |

### .VANI — vertex-animated object (grass, flags, water)
```
f32 center[3]; f32 rotY; f32 min[3], max[3]
i32 nItems; i32 nSteps; i32 features (render flags?)
  Item: str texture (Entity/Texture); i32 nFace; face[]; i32 nVertPerStep;
        {f32 pos[3], f32 normal[3], i32 bone (-1), f32 u, v}[nVertPerStep * nSteps]
f32 min2[3], max2[3]; i32 unk (0)
```
Frame `s` uses vertices `[s*nVertPerStep …]`; the same faces index into each frame. Loader `SModelAnimation::Load`
`FUN_00598290`; as in `.SMOD`, the final `i32` is a collision-mesh flag. **confirmed: 66/66** (79/79 in the original archive).

## Skinned meshes and animation

### .3DC — skinned character/monster mesh
Loaders `FUN_0053d5f0` (software skinning path) and `FUN_0053e850` (same reads, plus a bone palette for vertex shaders, see
Animation below).
```
str texture        empty in every file but Mob_Rend_01_A/B ("Mob_Rend_01.TGA"; the .MON record picks the DDS anyway)
                   (the published "tag 0 = format 5, 0x1BC = format 6" is this string's length; EP6's format is not checked here)
i32 nBones; f32 matrix[4][4] * nBones      last column always (0,0,0,1); inverse bind pose (see Animation)
i32 nVert; Vert[nVert]
   Vert: f32 pos[3], f32 weight, [f32 e, f (format 6 only)], u8 bone1, u8 bone2, u16 pad (0), f32 normal[3], f32 u, v
i32 nFace; face[]
```
All parts of one class/sex (upper, lower, hands, boots…) carry the same bone list (sometimes with extra bones at the end), so
pieces share one skeleton and coordinate system. Weight: bone1 only → 1.0; two bones → weight is bone1's share (0.5…0.84 seen);
the blend is in Animation below. The hierarchy is in the `.ANI` files (parent index per bone), not here.
**confirmed: 2431/2431** (2467/2467 in the original archive). All vertices are 40 bytes. The 48 cloak meshes (`cloak/…/*Mentle*.3DC`)
have **no bone block** (str, nVert, vertices, faces; their loader is not traced). `Npc/3DC/Npc_XXX_03.3DC` has a second, small mesh
(53 vertices, same 49 bones) appended; the client reads only the first.

### .ANI — skeletal animation
Loader `FUN_0053ffe0` (via `FUN_005403c0`; monsters/NPCs from `Data/<kind>/ANI`, characters through the `.ALT` list).
```
i32 unk0 (0; -1 in 7 files); i32 nKeys (last key + 1, see Animation); u16 nBones
  Bone: i32 parent (-1 = root; always a lower index); f32 matrix[4][4]  (bind pose in model space)
        i32 nRot;   {i32 key, f32 quat[4] (x,y,z,w)}[nRot]
        i32 nTrans; {i32 key, f32 vec[3]}[nTrans]
```
(Castor's version read `i16 maxKeyIndex; i16 unk1; i16 nSteps`: same bytes; his "boneIndex" is the **parent**.) The bone order is
the `.3DC` bone order. **confirmed: 3509/3509.** How the client applies it: Animation below.

### Animation: how game.exe plays .ANI on .3DC
Checked with the client repository's `tools/anim.py` (pure Python): `check` runs it over every monster/NPC/vehicle model and animation, `png`/`obj`
(and `png-files`/`obj-files` for characters) write a posed model. Posed monsters and characters come out whole and natural
(walk, run, attack), so the math below is enough to export models with working animations. Over all 5147 (model, animation)
pairs the skinned mesh keeps its size at every sampled time except 74: small parts (boots) that swing far relative to their size,
and `Mob_Pilate6` att01–03, whose bones 29–34 drop to y ≈ −240…−290 mid-attack (parts hidden by moving them under the ground).
Conventions are D3DX: **row vectors**, `v' = v · M`, translation in row 3, `A · B` = "A, then B".

**Load** (`FUN_0053ffe0`), per bone `i` with file matrix `Bind[i]` and parent `p`:
`InvBind[i] = Bind[i]⁻¹` (object +0x40), `Local[i] = Bind[i]` for a root, else `Bind[i] · Bind[p]⁻¹` (+0x80).
`Bind` is the skeleton in model space at the animation's first frame: for 169,193 of 169,778 bone tracks (99.7 %, every
monster/NPC/vehicle animation) the first rotation key, as a matrix, equals `Local` exactly. Parents always have lower indices, so one pass in file order works.

**Pose at time t** (`FUN_0053f740`; t in key units), per bone in file order:
1. No keys at all (`+0x114` flag clear): `L = Local[i]`.
2. Rotation: no rotation keys → `L = Local[i]` (rotation and translation). Otherwise find segment `k` (below);
   if `t < lastKey`: `q = Slerp(q[k], q[k+1], (t − key[k]) / (key[k+1] − key[k]))`, where for the last segment the
   client uses the header `nKeys` instead of `key[k+1]`; else `q = q[k]`. `nKeys` is the last key in 3391 of the 3509 files and
   larger in the others (by 1 in 85, up to 40): there the last rotation segment plays slower and the pose then holds until
   `t > nKeys`.
   `L = RotationQuaternion(q)` (D3DXMatrixRotationQuaternion, q = x, y, z, w; D3DX slerp takes the short way).
   The rotation **replaces** the bind rotation; it is not added to it.
3. Translation: no translation keys → row 3 of `Local[i]`; else the same segment search with a linear blend of the vectors,
   or the clamped key. Row 3 of `L` = that vector.
4. `World[i] = L` for a root, else `L · World[p]` (object +0xc0).

Segment search: `k` = the first key with `key > t − 2` (stopping at the second last), then `k − 1` if `t < key[k]` (not below 0).
This assumes keys at least 2 apart (true for every rotation track checked: keys are 0, 2, 4, …). For a bone whose first key
is later than `t` (some character emotes start a hand bone at key 96) the factor goes negative and the client
**extrapolates** the slerp; an exporter should clamp to the first key instead.

**Skinning** (`FUN_0053ddb0`; `FUN_0053d840`/`FUN_0053e2a0` do the same plus the entity's world matrix and bounds), per
vertex with `pos`, `weight w`, `bone1`, `bone2`, `normal`, and the `.3DC` matrices `B[i]`:
```
M  = w · (B[bone1] · World[bone1]) + (1 − w) · (B[bone2] · World[bone2])     (element-wise blend of the two matrices)
pos' = pos · M;  normal' = normal · M without row 3 (not renormalised)
```
`B[i]` is the mesh's **inverse bind pose** (the mesh-side `InvBind`): the client never uses the `.ANI`'s `InvBind` for skinning.
The mesh bind and an animation's first frame differ (different poses), so a single-bone vertex is not at rest at frame 0;
that is expected. A `.3DC` may carry more bones than an `.ANI`; vertices use only the shared ones. `FUN_0053e850`
(the vertex-shader path) loads the same file but renumbers the vertex bone bytes into a compact palette of used bones.

**Time** (monsters, `FUN_00431f70`): `t += dt · speed`, where speed (keys/second) comes from a per-monster byte: 15, 20, 25,
**30** (default), 35, 40, 45; attacks use `(attackSpeed + 1) · 10`. The animation ends when `t > nKeys` (header): it then
loops (walk, run, idle) or switches (attack → idle via a blend). With keys every 2, that is 15 keyframes a second at the default
speed, i.e. key units are 1/30 s. A hit is triggered at `t = nKeys / 2` for attacks.
**Blend between animations** (`FUN_0053fae0`): for 1/6 s (`b += 30 · dt` until `b > 5`) the pose is the old animation at t,
decomposed, then slerped/lerped towards the new animation's `Local` (its first frame) by `b · 0.2`.

### .MLT — model list for a character part ("ML2")
`char[3] "ML2"; i32 n3DC; str[n3DC]; i32 nDDS; str[nDDS]; i32 nRec; {i32 idx3DC, i32 idxDDS, i32 alphaFlag}[nRec]`.
alphaFlag 0: alpha = transparency, 1: alpha = glow. The record index is what item definitions reference. **confirmed: 112/112.**

### .ITM — weapon/item model list ("ITM" / "IT2")
`char[3] sig; i32 n3DO; str[]; i32 nDDS; str[]; i32 nRec; Rec[nRec]`,
Rec: `i32 idx3DO, idxDDS, glow, unk, format(0|1), unk` + if format==1: `i32 rgba, unk, scale, unk` + if "IT2": `i32[1024]`.
**confirmed: 17/17** (all "ITM"; IT2 is later episodes).

### .MON — monster / NPC / mount / gambling-NPC model list ("MO2")
```
char[3] "MO2"; i32 nRecords
  Record: str name; u8 flag (255 for 503/515 monsters; lower values on a few, likely opacity)
          str slot[17]          9 animation slots then 8 sound slots; "LOAD" = empty slot
                                monsters: walk, run, att1, att2, att3, die, br(eath/idle2), damage, idle; sounds: att1, att2, ?, die, …
          i32 nParts; {str .3DC, str .dds}[nParts]
          f32 size              (collision/selection radius or scale)
          i32 nAttach; {i32, i32}[nAttach]   e.g. (30, 66): likely bone index + effect id
```
Strings here have no trailing NUL. Files: `Monster/monster.MON` (515), `Npc/NPC.MON` (170), `Vehicle/vehicle_*_01.mon`,
`Strip/Strip_*.MON` (gambling-house NPCs). The record index is what `Monster.SData`/`NpcQuest.SData` reference.
**confirmed: 16/16.** `Npc/NPC.TOL` is the obsolete predecessor (cleanup candidate once confirmed unread).

## Effects

### .EFT — effect file: particle emitters ("scenes") and sequences
Loader `FUN_00546710`; emitter object built by `FUN_00549cc0` (defaults), updated by `FUN_0054a0f0` (in world space) /
`FUN_0054acd0` (attached to an animated 3DE), drawn by `FUN_00549990` → `FUN_005484d0` (billboards) or `FUN_00548070` (3DE
meshes); frame step `FUN_0054b930`. **confirmed: 446/446.**
```
char[3] "EFT"
i32 n; str 3de[n]            all loaded with the EFT, from data/Effect/3DE
i32 n; str dds[n]            data/Effect/DDS
i32 nScene; Scene[nScene]
i32 nSeq; {str name; i32 n; {i32 scene, f32 delay}[n]}[nSeq]   what the game plays (by index); delay: start offset, s (inferred)
Scene (emitter); [+xx] = offset in the emitter object
  str name                   skipped by the client
  i32 randVelX, Y, Z         [+e0..e8] 1 = pick a new random velocity on that axis every frame
  i32 loop                   [+d4] 1 = emit for ever, else for `duration`
  i32 destBlend              [+cc] index into the blend list below (mostly 1 = ONE: additive)
  i32 motion                 [+d8] 0 = straight; 1/2/3 = the velocity turns around Y/Z/X at `turnSpeed`
  i32 srcBlend               [+c8] mostly 4 = SRCALPHA
  i32 loopFrames             [+100] 1 = the frame list (and a 3DE's animation) loops, 0 = holds the last frame
  i32 mesh3de                [+fc] index into the 3DE list; -1 = flat billboards
  i32 onMesh                 [+17c] with a mesh: 0 = each particle is the 3DE mesh, 1 = billboards attached to the animated mesh
  f32 frameTime              [+188] seconds per entry of the frame list (default 1)
  f32 rateMax                [+54] particles per frame, random in [rateMin, rateMax]; fractions carry over; at most 200
  f32 lifeMax                [+c]  particle life in seconds, random in [lifeMin, lifeMax]
  f32 rateMin                [+50]
  f32 lifeMin                [+8]
  f32 duration               [+d0] emitting time when not looping (counts down)
  f32 turnSpeed              [+10] radians/second for motion 1–3 (default 10)
  i32 unk                    [+dc] 0, or 3.0 as a float in 111 emitters (not traced)
  f32 jitter[3]              [+b8] spawn position random in [-j, +j] per axis
  f32 gravity[3]             [+14] added to the velocity every second when `useGravity`
  f32 origin[3]              [+20] spawn position (relative to the effect)
  f32 velMin[3], velMax[3]   [+a0, +ac] start velocity, random per axis
  i32 facing                 [+108] billboard matrix: 0 = faces the camera, 1–3 = other precomputed view matrices
  i32 useGravity             [+10c]
  i32 useAttractor           [+110]
  f32 attractor[3]           [+114] particles accelerate towards this point ...
  f32 attractorForce         [+120] ... with this factor
  i32 randomSpin             [+124] start spin speed random in [0, spinSpeed]
  i32 spin                   [+128] 1 = the billboard rotates at spinSpeed ...
  f32 spinSpeed              [+12c]
  i32 spinAxis               [+130] ... around axis 1/2/3 (default 3)
  i32 n; {f32 r, g, b, a, f32 time}[n]     colour keys, linear in between (none: white)
  i32 n; {f32 factor, f32 time}[n]         velocity scale keys
  i32 n; {f32 min, max, time}[n]           size keys, random in [min, max] (none: 0.3)
  i32 mirrorUv               [+164] 1 = texture addressing MIRROR and quad UVs -1..2
  i32 angleAxis              [+168] 1/2/3: axis (X/Y/Z) of the start angle
  i32 angleMin, angleMax     [+16c, +170] start angle in degrees; min > max wraps through 360
  i32 n; i32 dds[n]          texture flip-book: indices into the DDS list, one every `frameTime`
```
Blend list (`D3DBLEND_*` names and values in a table at `0x6B61F0`, 32-byte records): 0 ZERO, 1 ONE, 2 SRCCOLOR, 3 INVSRCCOLOR,
4 SRCALPHA, 5 INVSRCALPHA, 6 DESTALPHA, 7 INVDESTALPHA, 8 DESTCOLOR, 9 INVDESTCOLOR, 10 SRCALPHASAT, 11 BOTHSRCALPHA,
12 BOTHINVSRCALPHA. A few indices in the files are out of range (5 mesh, 8 frame, 3 sequence indices); for frames the client
checks and draws untextured.

### .ALT — character action list ("AL3")
Loader `FUN_0053ef50`; one per race/sex/class group (`<race>/<xxxx>_action.ALT`, literals in `FUN_0053f230`). The slot number is
the action id.
```
char[3] "AL3"; i32 nSlots
  Slot: i32 len; if len == 0 the slot is empty and nothing follows; else char[len] .ANI name (in <race>/ANI), then
        u8 mode (1, 2; the other 3 bytes are uninitialised garbage), i32 (34 on attack/damage actions, else 0),
        f32 key[4] (attacks: e.g. 12, 17, 15: hit frames, inferred)
```
**confirmed: 16/16** (126 slots each).

## World

### .WLD — client map (world/<mapId>.wld)
Matches Parsec's `Wld.cs` (a public C# library for Shaiya files), which reads the always-empty `object` lists as three
unknown ints. Read by `game.exe` `FUN_004516f0` (called from `FUN_00452270`, which builds `%d.wld` from the map id at `0x84E6E0` and sends
`0x0201` when done). **confirmed: 83/83 parse to the last byte** (29 fields: 18 × 1024, 11 × 2048; 54 dungeons).
`name` = fixed 256-byte buffer, NUL-terminated; the bytes after the NUL are garbage from the original map tool (stack memory) and
must be ignored. `List(R)` = `i32 nNames; name[nNames]; i32 n; R[n]`, where each record points at a name by its **index**
(so editors must never delete a name: later indices would shift). `box` = `f32 min[3], max[3]`.
```
char[4] type                 "FLD\0" field or "DUN\0" dungeon
FLD:                         terrain (FUN_00446c50 sets up the grid; FUN_0058cfa0 reads it)
  i32 mapSize                1024 or 2048 (map 70 is forced to 1600 in memory)
  i16 height[n*n]            n = mapSize/2 + 1 corners per side (FUN_0059d3d0)
  u8  texIndex[n*n]          terrain texture per corner, index into the list below
  i32 nTex; {name texture (Terrain/Detail), f32 tiling, name footstep .wav}[nTex]    (FUN_0059cd70, 516 bytes each)
                             the footstep name picks the walk sound set: ch_walks* (stone), ch_walkg* (grass), else ch_walke*
  name water                 .wtr file in data/entity/water
DUN:
  name dg                    dungeon geometry (.dg in data/world/dungeon); its octree replaces the terrain
both:
  List(Placement) building   data/entity/building (.smod)          Placement = i32 nameIndex, f32 pos[3], f32 dir[3], f32 up[3]
  List(Placement) shape      data/entity/shape                     (40 bytes; dir/up give rotation and scale, -1 index = skip)
  List(Placement) tree       data/entity/tree
  List(Placement) grass      data/entity/grass
  List(Placement) vani       data/entity/vani (.vani)
  List(Placement) vani2      data/entity/vani, second list (much larger: animated grass/water)
  List(Placement) dungeon    data/world/dungeon (.dg pieces placed in a field, e.g. a cave mouth)
  List(Mani) mani            data/entity/mani; Mani = i32 index1, i32 index2, f32 pos[3], dir[3], up[3] (44 bytes)
  name effectFile            .EFT in data/Effect (e.g. world_a1.eft; L_portal.EFT when effects are off)
  i32 n; {f32 pos[3], dir[3], up[3], i32 sequence}[n]           placed effects (FUN_00450450)
  List(Obj) object           data/entity/object (FUN_00523900): Obj = i32 index, f32 pos/dir/up[9], 9 × 4-byte params (76 bytes)
     i32 nPairs; {Obj, Obj}[nPairs]                                 linked pairs; both lists are empty in every EP4 map
  List(Placement) object2    data/entity/object (FUN_00524230)
  sound (FUN_00529fd0):
     i32 nBgm; name bgm[nBgm];   i32 n; {box, f32 rotY, i32 bgmIndex, i32 flag}[n]        music zones (36 bytes)
     i32 nWav; name wav[nWav];   i32 n; {box, i32 k, i32 wavIndex[k]}[n]                  ambient-sound zones
     i32 n; {i32 wavIndex, f32 pos[3], f32 radius}[n]                                     sound spots (20 bytes)
  zones (FUN_005258d0):
     i32 n; {box, f32 rotY}[n]                                      oriented boxes (rare; Parsec calls them monster-restricted zones)
     i32 n; Portal[n]            Portal (556 bytes) = box, f32 rotY, name, char[248] destination name (shown in the UI),
                                 i32 country (1/2 seen), i16 minLevel, i16 maxLevel (0, 999), i32 destMap, f32 destPos[3]
                                 (must match the server .svmap portals; the L_portal effect is drawn at the box centre)
     i32 total; groups until `total` records are read: {i32 k; {box, f32 rotY, i32, i32}[k]}   (Parsec: spawns; not traced)
     i32 n; Area[n]              named areas (548 bytes) = box, f32 rotY, name, name tga, i32 mode, i32; their order is the
                                 index into world/<mapId>.txt (mode 1 = text caption, 2 = .tga caption)
     i32 n; then records while i < n, each {i32 npcType, i32 npcTypeId, f32 pos[3], f32 angle, i32 k, f32 pos[3][k]}
                                 and the client subtracts k from n after each one. NPC positions used for the minimap and quest
                                 marks (FUN_005254e0 looks the NPC up by type/typeId as in NpcQuest.SData, types 1–13);
                                 the NPCs themselves are spawned by the server (0x0E01)
FLD: name sky[3]             sky dome .bmp and two cloud .tga (data/sky); dungeons have none (maps 39/40 use fixed names)
both: f32 rgb[3] ×3          three colours (0..255): the first is kept as a D3D colour value, the second as ARGB, the third
                             feeds the fog/light settings when feature 0x26 is on (exact roles not traced; Parsec: 2 unused + fog)
      f32 fogNear, fogFar    read but not used (e.g. 300, 600)
```
Rendering: each terrain tile is 4 triangles (`/wire` GM command shows it), diagonals generated by the client. After the file is
loaded the client reads `DATA\world\<mapId>.txt` (area names). Height scale reported by another member: `(i16)h * 0.2 - 200.0`
(*inferred*). The client also probes `Data/World/<id>.svmap` (`FUN_00430280`, only when a feature flag is set): optional.

### .DG — dungeon geometry (world/dungeon/*.dg)
Read by `FUN_0058fba0` (`SDungeonScene::Load`). **confirmed: 46/46 parse to the last byte.**
```
box bounds
i32 nTex; name texture[nTex]      data/entity/texture
i32 nLightmaps                    <dg name>_L0.dds … _L(n-1).dds next to the .dg (FUN_0058d820)
i32 hasRoot; Node (if hasRoot)
Node (octree, FUN_0058f860):
  f32 center[3]; box; box
  i32 nGroups; {i32 textureIndex; i32 nMesh; Mesh[nMesh]}[nGroups]
     Mesh = i32 lightmapIndex; i32 nVert; Vert[nVert]; i32 nFace; face[nFace]
     Vert = f32 pos[3], f32 normal[3], u32 colour (0xFFFFFFFF), f32 uv[2], f32 lightmapUv[2]   (44 bytes, FVF 0x252)
  i32 hasCollision; if set: i32 nVert; f32 pos[3][nVert]; i32 nFace; face[nFace]
  8 × {i32 hasChild; Node (if hasChild)}
```

### .SVMAP — server map (read by server tools, listed here for reference)
`i32 mapSize; u8 heights[mapSize*mapSize/8]; i32 unk; Ladders; MonsterAreas; NpcLocations; Portals; Spawns; NamedAreas`.

## SData (client tables)

See `data-archive.md` for the SEED wrapper. Layouts known from the sticky: `KillStatus.SData` (`i32 count; {bool country,
i32 bless, i16 index, (u8 type, i16 value) x6}`), `GuildHouse.SData` (`i32 unk, homePrice, servicePrice; npcInfo[36]
{u8 priceRate, lvl, mixRate, mixDecrease, minRank; i16 icon, msg, upPrice, servicePrice; u8 npcType, group}; i32 npcId[24]`).
Item/Skill/Monster/NpcQuest: Parsec has full EP4–EP6 layouts (its `Parsec/Shaiya` sources).
`Item.SData` (EP4) = 100 "banks" (item Type 1..100), each 255 records in TypeID order; unused records are zero, never removed,
because ps_game identifies an item by its row position, not by the stored Type/TypeID. Gears/weapons pick their look by index into
the `.MLT`/`.ITM` lists. In the EP5 layout the item's skill-ID field is 1 byte (shStudio adds 256 to values below 45).
`NpcSkill.SData`/`Skill.SData` EP4 = 3 levels per skill (EP5: 9, EP6: 15). `PriestTalk.SData` (Korean text, length errors) is very
likely unused by the client. `.MON` (monster/NPC/mount model lists): EP4 format still to check (EP5 "MO2", EP6+ "MO4"); `.TOL` is the
obsolete predecessor (cleanup candidate: `Npc/*.tol`).

## Archive notes (Castor)

- Only SData are encrypted (SEED, fixed key); maps and models are plain, the archive itself is not protected.
- `data.saf` is the plain concatenation of files; `data.sah` holds name, offset, length (see `data-archive.md`).
