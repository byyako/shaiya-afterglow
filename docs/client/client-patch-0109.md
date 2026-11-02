# Client data patch 109

Batch 1 only. **Verified (files) and live (updater):** the hand-over passed independent client-side parsing and record comparison; the two-member patch was packed, signed and installed by the real updater on an isolated spare. A one-byte-altered copy was refused. **Live (game):** all three stone/buff displays, consumption, upward replacement and weaker-stone refusal passed on the spare. The resumed spare check also passed same-tier refresh for 200/1, direct Mini Sun icon comparison, and unchanged weapon/potion tooltip spot-checks. Death/relog persistence and player-kill increments remain pending; see acceptance below. This is not a release claim.

## Scope, source identities and evidence

- **Verified (files):** input is the server's `data/client-patches/ps0109/` (`manifest.json`, `changes.txt`, `Item/Item.SData` and `Character/Skill.SData`), read from the server's `data/client-patches/ps0109/`. Manifest: patch 109, previous 108, `dirty: false`, and a generation commit that exists and describes the three-strength kill-boost and generator hand-over. The server working tree has subsequent unrelated changes; this report does not certify its present generator or database state.
- **Verified (files):** original client, the client repository's `game.exe`, SHA-256 `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`; its archive is still at 108 (`Version.ini`). The original executable was not changed or run. The everyday archive was read directly, not trusted from an older extraction.
- **Verified (files):** client readers used were the client repository's `tools/sah.py`, `tools/sdata_crypt.py`, `tools/sdata_diff.py`, plus the existing viewer's `sdata-viewer/generated/Schemas.cs` to label the skill fixed fields. No server SEED implementation, generator, database or server tool was used to establish the binary comparisons.
- **Verified (files):** all six required background documents were read: `docs/client/README.md`, `CLIENT-FACTS.md`, `sdata-generation.md`, `gm-and-commands.md`, `research-gambling-kill-boost.md`, and `docs/plans/done/client-patches.md`. Batch 1 names no research-queue entry. The patch specification's section 5 and section 51's kill-boost checklist govern the acceptance below.
- **Verified (files):** only this report is intended for the NewShaiya commit. Client release and staging files remain on the client side's machine. No server code, database, queue, checklist, README or patch ledger was changed. No patch was published or pushed. The temporary staging listener bound only `127.0.0.1` and was stopped.

Statuses: **verified** = inspected code/data, **inferred** = conclusion from that evidence, **live** = observed execution. Source paths below are relative to the client repo unless prefixed NewShaiya. VAs, where used, are `game.exe` virtual addresses, not file offsets.

## 1. Independent hand-over verification

**Verified (files):** read the two old encrypted members directly using the active `data.sah` index and `data.saf`. Each old hash equals `replaces_sha256`. The independent client SEED reader checked CRC-32; both plaintext hashes and lengths equal the manifest. Both client record readers required exact EOF. All record keys and counts are unchanged: Item 100 groups / 16,045 records; Skill 326 IDs / 978 level records. The item reader's dictionary row is zero-based; its row 211 means TypeID 212.

| Member | 108 SAF offset | 108 encrypted bytes / SHA-256 |
|---|---:|---|
| `Item/Item.SData` | `0x3394ea7c` | 2,960,192 / `bb1ef0fccfc7d80be854b5376d18e41234920921ed62d791dc4a9ac78321ef48` |
| `Character/Skill.SData` | `0x49cbb8` | 221,984 / `0ff8450aa07ddf1a6eace85bf61435f44e3699ccdaa722fe10703bf8d09a8d92` |

| Member | 109 encrypted bytes / SHA-256 | 109 decrypted bytes / SHA-256 |
|---|---|---|
| `Item/Item.SData` | 2,960,592 / `a92292782764918ec1e6b8b929fe66617d340da183ec4664eba46c80ddf47bea` | 2,960,527 / `505037949981b32fe9fadc1d341d90ddda474c27683d5752615413eadd4228ce` |
| `Character/Skill.SData` | 222,368 / `6c5b4bc42032949494efd97a226008c6af9f44df9d52b211c0fc5ca5fb54649c` | 222,302 / `3f110f762e40349d6a9308d74aa967994b0e8f42d6f1e8b90a08f77cf6d12b7e` |

**Verified (records):** only Item 100/212, 100/213, 100/214 and Skill 200/1, 200/2, 200/3 differ. Every reported old/new value in `changes.txt` was compared to the client-parsed records, and every changed byte of each fixed block had to belong to a reported field. Result: exactly 10 fields per item and 34 per skill; no unreported changes. The retained field-by-field result is `release/ps0109/staging/record-differences.json`.

| Records | Field differences verified against `changes.txt` |
|---|---|
| Item 100/212..214 | name `???177` -> respective stone name; description `???` -> respective exact text below; model 0 -> 1; icon 1 -> 8; country 0 -> 6; range 0 -> 200; attacktime 0 -> 1/2/3; sell 0 -> 1000; grade 0 -> 545; count 0 -> 1 |
| Skill 200/1..3 | name `Blank` -> respective buff name; description `Blank` -> exact text below; icon 0 -> 225; country 0 -> 6; six class flags 0 -> 1; type_effect 0 -> 4; type_detail 0 -> 82; need_weapon1..15 and shield 0 -> 1; reset_time 0 -> 1; success_type 0 -> 1; success_value 0 -> 100; target_type 0 -> 2; keep_time 0 -> 1; fix_range 0 -> 1 |

**Verified (records):** names are `Kill Boost Stone`, `Kill Boost Stone Lv2`, `Kill Boost Stone Lv3` and `Kill Boost`, `Kill Boost Lv2`, `Kill Boost Lv3`. Item and matching skill descriptions are identical, exactly:

- 100/212 and 200/1: `Player kills count double for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.`
- 100/213 and 200/2: `Player kills count 5 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.`
- 100/214 and 200/3: `Player kills count 10 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.`

**Verified (records):** each item's model/icon and grouping bytes equal old 100/108; ordinary-use Special is 0, the skill-link byte is 200, and the level is 1/2/3. Each skill's icon/animation/toggle/sound bytes equal old 250/1; icon is 225; KeepTime is 1 and duration flag is 1; all three ability type/value pairs remain zero. All other item/skill records, including 100/108, are byte-equivalent at record level. These checks establish data, not rendered appearance, server multipliers, consumption or persistence.

**Verified (Ghidra, local-copy evidence):** lookup `0x461ba0` requires a nonzero skill ID at most the loaded u16 count, a non-null skill row, and level 1..3, then selects its `0x88`-byte level record. This was rechecked on the hash-identical original game.exe copy. **Prior verified code references**, corroborated by the game observations below: buff application `0x56c020`, long-duration update `0x4b8710`, icon draw `0x49dfd0`; source NewShaiya `docs/client/research-gambling-kill-boost.md`, "Item use, buff duration, icon and text". The remaining decompilation statements are prior evidence, not newly re-run routines.

**Verified (commands):** the core independent verification was run from the client repo with `python -u -B -I -`, passing this code on stdin (the field-by-field extension also used the existing Skill schema and checked every `changes.txt` field as described above):

```python
import pathlib, sys, json, hashlib
sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path.cwd() / 'tools'))
import sah, sdata_crypt, sdata_diff
handover = pathlib.Path(r'<the server data folder>\client-patches\ps0109')
manifest = json.loads((handover / 'manifest.json').read_text())
assert manifest['dirty'] is False
assert (manifest['patch'], manifest['previous']) == (109, 108)
sha = lambda data: hashlib.sha256(data).hexdigest()
index = sah.read_sah('data.sah')
with open('data.saf', 'rb') as archive:
    for spec in manifest['files']:
        matches = [e for e in index.entries if e.path.lower() == spec['member'].lower()]
        assert len(matches) == 1
        entry = matches[0]
        archive.seek(entry.offset)
        old = archive.read(entry.length)
        new = (handover / spec['member']).read_bytes()
        assert sha(old) == spec['replaces_sha256']
        assert (len(new), sha(new)) == (spec['size'], spec['sha256'])
        before, after = sdata_crypt.decrypt(old), sdata_crypt.decrypt(new)
        assert (len(after), sha(after)) == (spec['plain_size'], spec['plain_sha256'])
        item = spec['member'].startswith('Item/')
        parse = sdata_diff.parse_items if item else sdata_diff.parse_skills
        a, b = parse(before), parse(after)  # both readers assert exact EOF
        assert a.keys() == b.keys()
        changed = [f'{k[0]}/{k[1]+1 if item else k[1]}' for k in a if a[k] != b[k]]
        assert changed == spec['changed']
        print(spec['member'], len(a), changed, 'PASS')
```

## 2. Pack, sign and retain

**Verified (files):** output directory is the client repository's `release\ps0109\`. The ZIP contains only `update.sah` and `update.saf`. The embedded SAH contains exactly the two manifest member paths. ZIP CRC, member payload bytes and embedded archive metadata were checked before signing. Patch-tool listing: `2 archive files, 0 loose files, 0 removals`.

**Verified (source/data):** `rust/archive/src/patch.rs::build` (line 190) takes each input file's modification time as the archive entry version. The input copies' timestamps were therefore set to the old members' versions before packing: Skill 1248771636, Item 1274079412. Both embedded and installed versions were checked equal to the old versions. This preserves metadata and makes the before/after comparison limited to offsets, sizes and contents.

Commands, run from the client repo in PowerShell:

```powershell
# Copy only the manifest's two members into release/ps0109/inputs/{Item,Character}.
# Set each input file's Unix mtime to its old SAH entry.version before build.
.\rust\target\release\shaiya-patch.exe build .\release\ps0109\ps0109.patch --files .\release\ps0109\inputs
.\rust\target\release\shaiya-patch.exe list .\release\ps0109\ps0109.patch
.\rust\target\release\shaiya-patch.exe sign C:\Users\silen\.shaiya-keys\patch-secret.key .\release\ps0109\ps0109.patch --check-version 3
Get-FileHash .\release\ps0109\ps0109.patch, .\release\ps0109\ps0109.patch.sig, .\release\ps0109\UpdateVersion.ini -Algorithm SHA256
```

**Verified (files):** the existing private key was passed directly to `shaiya-patch sign` on the client side's machine. Its contents were neither printed nor copied. The normal compiled public key verified the resulting signature in the real updater; no unsigned override or throwaway key was used. Signature implementation: `rust/archive/src/sign.rs`, `sign` line 73, domain/name/check-version message at line 23.

| Retained release file | Bytes | SHA-256 |
|---|---:|---|
| `ps0109.patch` | 2,052,649 | `9478d66a773db74ac132d8af13a40fb727e6564a9abdf3cb99532e3e2c17bde5` |
| `ps0109.patch.sig` | 129 | `9d27d3f161c3d783ea5e42378d76e73b6edefc99119c92c7508390477f4f9533` |
| `UpdateVersion.ini` | 49 | `e69e0ef501d5d2c8d71f1096d32002748ac228bc522060fd513b864eb46ae6c5` |

**Verified (file):** the version file has CRLF, UTF-8 without BOM, no self-updater offer:

```ini
[Version]
CheckVersion=3
PatchFileVersion=109
```

**Verified (files):** existing binaries used, without rebuilding unrelated source changes:

| Binary | SHA-256 |
|---|---|
| `rust/target/release/shaiya-patch.exe` | `703e0bb12dde4e4a26224979b41eaae21682f93bed93033d2bf692fe31a9617f` |
| `rust/target/release/shaiya-updater.exe` | `87c5288c143109ed9d6889a32861409231ea38ef25ad68447d246e15bb5c6191` |

## 3. Real-updater staging results

**Verified (files):** spare client is `release/ps0109/staging/client/`, with an independent copy of the current cleaned patch-108 archive and Version.ini. For possible dev-server gameplay it also has a copy of existing `game_local2.exe` named `game.exe` (SHA-256 `d666833b40e74072703df9d53d8a09b4cd55f802446cc05ce7cf14ae343d28c8`), the existing DLL/settings and `ijl15.dll`. This is a dev executable, distinct from the pristine game.exe used for static identity. The existing release updater was copied to the spare as `Updater.exe`.

**Live (updater):** a Python standard-library `ThreadingHTTPServer(('127.0.0.1', 0), ...)` served only `release/ps0109/staging/http/`. Assigned address was `127.0.0.1:51536`; the spare's `UpdaterURL.txt` held that bare `host:port` plus CRLF. Layout:

```text
staging/http/Shaiya/UpdateVersion.ini
staging/http/Shaiya/patch/ps0109.patch
staging/http/Shaiya/patch/ps0109.patch.sig
```

**Live (updater):** the same real executable was run twice, first with one byte changed in the staging copy and the authentic signature unchanged, then with the authentic patch restored:

```powershell
& '.\release\ps0109\staging\client\Updater.exe' --dir '.\release\ps0109\staging\client' --cli --no-launch --no-pause --no-self-update
```

**Live (updater):** first run exited 1 with `signature does not match (file altered, or signed for another name/version or with another key)`. The changed byte was offset 1,026,324, XOR 1. Damaged copy SHA-256: `34b1041b16713b16c68a71885a9494a11a58d583367b233327c6d4862ffa6aed`. Both spare archive files retained their complete baseline hashes; CurrentVersion remained 108. `StartUpdate=UPDATE_START` recorded the interrupted attempt, as expected from `rust/updater/src/main.rs::run`.

**Live (updater):** second run exited 0, reported recovery from 108, fetched and verified the valid patch/signature, and logged `0 added, 2 replaced, 0 removed, 0 loose files`, then `up to date (version 109)`. The spare now has `CheckVersion=3`, `CurrentVersion=109`, `StartUpdate=UPDATE_END`. HTTP logs show all three paths fetched successfully for each run. The listener was shut down and joined after the two runs.

**Verified (archive after execution):** before/after SAH both contain the same 19,939 paths in the same order. Header version, file count, free list, trailing bytes and every unaffected entry are equal. Exactly these two entries differ; each entry's version remains unchanged:

| Member | Old offset / bytes | New offset / bytes | Installed payload |
|---|---|---|---|
| `Character/Skill.SData` | `0x49cbb8` / 221,984 | `0x76bad172` / 222,368 | exact manifest encrypted hash |
| `Item/Item.SData` | `0x3394ea7c` / 2,960,192 | `0x76be3612` / 2,960,592 | exact manifest encrypted hash |

**Verified (archive bytes):** the first 1,991,954,802 bytes of the updated spare SAF hash exactly as the complete old SAF. Thus the original byte ranges backing every unchanged member are intact; only the two new payloads were appended. No compaction occurred. The everyday SAH and SAF were hashed before and after staging and remained at the following baseline values:

| File | Before bytes / SHA-256 | Updated spare bytes / SHA-256 |
|---|---|---|
| `data.sah` | 771,851 / `b3a6338f51cbd82a7342ae95624de075fbf13a0b8651de8e2ad131937fd05140` | 771,851 / `9d5e176c13c9c9ed21208ce0980d3a5de9c49f607059362050608f5a1c820a88` |
| `data.saf` | 1,991,954,802 / `d2f250823d280a1206edebfed15414199b1ff05725507c363408d01929500e77` | 1,995,137,762 / `a1730164b30260ef192432e509199a6d7bb501c2bb2ea6fb016a9c905ee5de5c` |

**Verified (evidence files):** `release/ps0109/staging/results.json` records before/after hashes, exact updater arguments, status/exit codes, requests, entry changes and the stopped-listener flag. `tampered-updater.log` and `valid-updater.log` retain console output. The staged HTTP tree now holds the authentic patch. Its old ephemeral UpdaterURL address is evidence of this run, not an active service.

## 4. In-game acceptance

**Live (game):** after the initial launch approval timed out, the maintainer opened the exact spare and signed into **GMTest**, level 60 Fighter, then explicitly handed over its controls. The returned window identified `process:<the client repository>\release\ps0109\staging\client\game.exe`; observations below are from that spare, not another installation. The game remained open at the end of that first observation pass; the maintainer restarted the spare before the resumed checks below.

**Live (commands):** `/getitem 100 212 1`, `/getitem 100 213 1`, `/getitem 100 214 1` each produced the corresponding `Attained Kill Boost Stone...` line and one item in bag II, slots 6, 7, 8 (zero-based wire slots). The documented numeric spelling `/getitem` was used; a preliminary `type_text` attempt to enter the requested `/imake` spelling produced no text and was not submitted, so `/imake` numeric behavior was not live-tested. Normal key events worked. Source: spare `Log.txt` lines 5657..5693 and retained `staging/stone-packets.json`.

| Test (live) | Result and exact record / packet reference |
|---|---|
| Item names and text | All three inventory tooltips visibly matched their exact strings in section 1. Their round purple/gold stone art matched one another and the expected reused art; direct comparison with newly created 100/108 passed in the resumed run below. Item creation packets: 100/212 at log 5661; 100/213 at 5677; 100/214 at 5689. |
| 200/1 buff | Used bag 2/slot 6 at log 5697; `050D key=1,id=200,level=1` at 5705; `050A ... type=100,id=212,count=0` at 5709. Stone disappeared. Tooltip `Kill Boost`, exact double text, reused purple/gold buff icon, timer `00:59:44` at observation. |
| 200/2 buff and upward replacement | Used 2/7 at 5717; `050E key=1` at 5725, `050D key=2,id=200,level=2` at 5729; consumed at 5733. Only one kill-boost icon remained. Tooltip `Kill Boost Lv2`, exact 5-times text, timer `00:59:53`. |
| 200/3 buff and upward replacement | Used 2/8 at 5737; `050E key=2` at 5745, `050D key=3,id=200,level=3` at 5749; consumed at 5753. Tooltip `Kill Boost Lv3`, exact 10-times text, timer `00:59:51`. Its existing icon is the same cell verified as 225 in the data. |
| Weaker refused, retained | Created another 100/212 at 5757..5765 while Lv3 ran; used 2/6 at 5773. Failure `050A` with zero bag at 5777; chat showed `You do not meet this item's requirements`. The weaker stone remained in its slot and the Lv3 buff remained active. |

**Live (resumed spare):** after the maintainer unlocked Windows and restarted, the exact spare executable above was selected again and the maintainer manually signed GMTest in. The original first-pass `stone-packets.json` is retained. The restarted client reset `Log.txt`, so the following line references belong to a separate, whitelisted excerpt, `release/ps0109/staging/resumed-stone-packets.json` (14 packets, 10:23:48..10:28:35). Authentication and unrelated packets are excluded.

| Resumed test (live) | Result and file/record reference |
|---|---|
| Mini Sun reference | `/getitem 100 108 1` created Mini Sun EXP Stone in bag II, slot 7 (`F702` line 573, `0205` line 577). Its inventory tooltip showed its original one-hour EXP description. Its purple/gold inventory art visibly matched adjacent Kill Boost Stone 100/212. The Mini Sun was left unused. Item 100/108 was already independently verified unchanged, model 1/icon 8. |
| Unchanged weapon | Bag I Long Sword, Item 1/1: its ordinary tooltip rendered level 1, attack 19-21, normal attack speed, endurance 1200/1200 and price 100, with the original Fighter/Defender/Archer description. The equipped comparison showed 1199/1200 endurance. Record 1/1 is unchanged by the patch. |
| Unchanged potion | Bag I Etain Potion, Item 100/1: tooltip rendered `Restores HP, SP, and MP by 75 percent each.` This exact spelling identifies 100/1; the similarly named 100/91 has different text. Record 100/1 is unchanged by the patch. These weapon/potion checks are current rendering plus independent record equality, not a side-by-side live 108 screenshot comparison. |
| Same-tier refresh, 200/1 | Used the retained 100/212 at 10:24:51: `050A` line 589, `050D key=4,id=200,level=1` line 597, consumption line 601. Created another 100/212 at 10:28:26 (lines 617..625). Before its use the visible timer was `00:56:24`. Used it at 10:28:35 (line 629); the server reapplied the same key/ID/level in `050D` line 637 and consumed the stone at 641. No `050E` was sent in this refresh sequence. One Kill Boost icon remained, exact tooltip text remained, and the newly observed timer was `00:59:48`: a fresh hour, without stacking the previous remaining time. This same-tier test covers 200/1; equal-tier refresh for 200/2 and 200/3 was not separately exercised. |

**Live (coverage boundary):** no death or deliberate relog persistence test and no player-kill test has been performed, and no second player/bot has been used. The overnight interruption and subsequent login do not prove offline persistence: the old hour had elapsed. The initial display/use/refusal results and resumed checks above remain distinct from those pending server-behavior gates.

Cheapest exact acceptance sequence, on this spare only, against the existing dev server:

1. Confirm the spare `Version.ini` says 109. Launch `release/ps0109/staging/client/game.exe`, log in with a dev GM, and enter world.
2. Try the requested `/imake 100 212 1`, `/imake 100 213 1`, `/imake 100 214 1`. If the stock parser treats `/imake` as name-based, use its documented numeric command `/getitem 100 212 1` (then 213, 214), or `/imake "Kill Boost Stone" 1` and the corresponding Lv2/Lv3 names. Source: `docs/client/gm-and-commands.md`, F702 row; parser `0x4705d0`. Record the actual successful spelling and packets.
3. Hover each stone. Verify its exact name/text above, its Mini Sun EXP Stone icon, and one item in the bag. Compare 100/108, one existing weapon and one potion visually with a 108 view; record the actual weapon and potion identities.
4. Use the stones in ascending strength. For each, verify consumption, a single matching buff with the exact name/text, icon 225 and approximately one hour. Confirm higher replaces lower with a fresh hour. Try lower while higher is running: refusal without losing the lower stone. Reapply equal strength: fresh hour rather than accumulated time.
5. Confirm death retention and relog retention with elapsed offline time deducted. Use existing normal dev-server controls; do not edit database rows or restart/change server settings for this test.
6. If another player/bot is available, perform section 51's non-duel PvP checks at the existing known kill rate: baseline one rate unit, then 2, 5, 10 units with 200/1..3. Record before/after character counts, the configured rate, buff identity and the second participant. No second participant is presumed available. The client file alone cannot prove these server calculations.

## 5. Answers for later serving

1. **Yes, the current custom updater accepts bare `host:port` (live).** This staging run used exactly `127.0.0.1:51536` in UpdaterURL.txt and fetched both the INI and signed patch. **Verified (source):** `rust/updater/src/main.rs::base_url`, line 173, takes the first nonempty trimmed line and prefixes `http://` unless a full HTTP/HTTPS URL is present. This answer concerns the custom updater whose hash is recorded above; the retained 2009 updater was not tested.
2. **No missing patch-format prerequisite was found in this hand-over (verified files/live updater).** The manifest's `member` paths, complete encrypted files, previous hashes and `changes.txt` were sufficient to pack and install the exact replacements. Use `Item/Item.SData` and `Character/Skill.SData` as archive members; the manifest's `us_path` values are history references, not an extra `data/` directory to pack. The client side supplies the SAH/SAF ZIP envelope, signature and UpdateVersion.ini. The signing key/public-key choice stays with the updater on the dev PC. Gameplay acceptance and later hosting are separate outstanding gates, not missing format inputs.

## Tooling coverage

**Verified (tool results):** the reverse-engineering tool server was absent, so the existing Ghidra and JDK installs were supplied directly, and only to the analysis process; no registration or machine configuration was changed. A direct analysis on a mapped network drive failed with Win32 error 87. Hash-identical local copies of game.exe and fixes.dll were made in a scratch folder; originals remained untouched.

**Verified (Ghidra):** the first local-copy attempt timed out at the 330,000 ms startup deadline; a longer startup timeout (900,000 ms) succeeded for `game.exe` VA `0x461ba0`, Ghidra 12.1.4. The analysis ran on a temporary database, deleted on close, and no original bytes were written. Original names/types may be missing (GAME.pdb was not loaded); decompiler output is not original source. The hash-identical existing fixes.dll copy also completed Ghidra analysis: export `fixes_init` returns 1; DLL SHA-256 `e1910d289438f65de49845982efed8ac78720d84cb4455852fe70991863e638c`. This bounded export check is not a claim that every DLL feature was re-audited or installed anew. The independent SData/packing checks remain client Python/Rust-tool and real-updater evidence, distinct from the Ghidra reads.

## Files, commit and remaining gates

**Verified (files):** NewShaiya write: only `docs/client/client-patch-0109.md`, CRLF. Outputs on the client side: the three release files listed above; two encrypted build inputs; the isolated spare, HTTP staging copies and staging evidence files under the client repository's `release\ps0109\`; hash-identical local analysis copies and result files in a scratch folder. No key was copied.

| Not yet settled | Cheapest confirming check |
|---|---|
| Side-by-side live 108 appearance comparison | Current 109 Mini Sun, Long Sword and Etain Potion tooltips passed; records compare unchanged. If a before/after visual pair is required, capture these same three records on a separate 108 spare and compare. |
| Death and relog behavior; equal-tier refresh at levels 2/3 | Steps 4..5 above on the prepared spare. Level-1 refresh, consumption, stronger replacement and weaker refusal already passed. |
| 2x/5x/10x player-kill increments | Section 51 with one cooperating player/bot, step 6 above. |


Batch 1 has no [inq] feature items; no feature-size table or shared implementation mechanism applies. Shared existing packaging mechanism: the two generated files -> SAH/SAF ZIP -> Ed25519 signature -> normal updater replacement.

Next batch: 2 (limits, extension points, the archive), say go.
