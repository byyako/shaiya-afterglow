#!/usr/bin/env python3
"""Developer tool: assemble the server patches and emit fixes.json (the byte lists
patch_ps_game.py applies). It is NOT needed to apply the patches; it rebuilds the
byte lists from the fix definitions below, using the GNU assembler, and checks each
fix's "original" bytes against the real ps_game.exe when that file is available.

usage: build_fixes.py <ps_game.exe>   # verifies originals, writes fixes.json
       build_fixes.py                 # rebuilds fixes.json without verifying

Nothing is executed. Caves are assembled at their final virtual address so that
jump displacements are correct, then disassembled back for the report.

See docs/original/server-patch-notes.md for the per-fix write-up.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

EXPECTED_SHA256 = "0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547"
SIZE = 1323008
IMAGE_BASE = 0x400000

# The code section (.text) is mapped from file offset 0x1000 at virtual address
# 0x401000, so VA = file offset + 0x400000 across the whole section, including the
# zero padding after the last function. VA -> file offset is therefore VA - 0x400000.
def va_to_off(va):
    return va - IMAGE_BASE

# --- the code cave -----------------------------------------------------------
# .text's raw data runs to file offset 0x121000; the last real byte is at file
# 0x120244 (VA 0x520244), leaving 0xDBC zero/pad bytes mapped and executable. We
# place caves from VA 0x520250 (keeping the 12 byte 0xCC run at 0x520244 intact).
CAVE_START = 0x520250
CAVE_END = 0x521000  # exclusive

# VirtualSize of .text, field at file offset 0x200 (little-endian u32). The caves
# live past the current VirtualSize (0x11F244) but inside the raw section, so they
# are already mapped; raising VirtualSize to 0x120000 (RVA end 0x121000) makes the
# mapping explicit. Applied when any cave fix is selected, reverted when none are.
VSIZE_OFF = 0x200
VSIZE_ORIG = (0x11F244).to_bytes(4, "little")
VSIZE_PATCHED = (0x120000).to_bytes(4, "little")


def asm(text, va):
    """Assemble intel-syntax `text` as 32-bit code located at `va`; return bytes."""
    src = ".intel_syntax noprefix\n.text\n" + text + "\n"
    with tempfile.TemporaryDirectory() as d:
        s, o, b = (os.path.join(d, n) for n in ("a.s", "a.o", "a.bin"))
        open(s, "w").write(src)
        subprocess.run(["as", "--32", "-o", o, s], check=True)
        subprocess.run(["ld", "-m", "elf_i386", "-Ttext=0x%x" % va, "--oformat",
                        "binary", "-o", b, o], check=True, stderr=subprocess.DEVNULL)
        return open(b, "rb").read()


class Builder:
    def __init__(self):
        self.cave = CAVE_START
        self.fixes = []

    def alloc(self, n):
        va = self.cave
        if va + n > CAVE_END:
            raise SystemExit("code cave exhausted")
        self.cave += n
        return va

    def inplace(self, va, original_hex, patched_hex):
        o = bytes.fromhex(original_hex)
        p = bytes.fromhex(patched_hex)
        assert len(o) == len(p), "in-place patch changes length at %x" % va
        return {"va": va, "off": va_to_off(va), "original": o.hex(), "patched": p.hex()}

    def hook(self, hook_va, hook_len, cave_asm, resume_va):
        """Redirect `hook_len` bytes at `hook_va` to a freshly allocated cave that ends
        with `jmp resume_va`. `cave_asm` is the cave body (displaced instructions plus
        the fix); it must not contain the trailing jmp. Returns (hook_patch, cave_patch)."""
        assert hook_len >= 5, "need 5 bytes for a near jmp at %x" % hook_va
        # First pass: assemble the body at a provisional address to learn its length.
        body = asm(cave_asm, CAVE_START)
        total = len(body) + 5  # + jmp resume
        cave_va = self.alloc(total)
        full = asm(cave_asm + "\njmp 0x%x\n" % resume_va, cave_va)
        assert len(full) == total, "cave length drift at %x" % cave_va
        patch = asm("jmp 0x%x\n" % cave_va, hook_va) + b"\x90" * (hook_len - 5)
        return (
            {"va": hook_va, "off": va_to_off(hook_va), "patched": patch.hex()},
            {"va": cave_va, "off": va_to_off(cave_va), "patched": full.hex(),
             "fill": "cc" if cave_va == CAVE_START else "00"},
        )

    def hook_raw(self, hook_va, hook_len, cave_asm):
        """Like hook(), but `cave_asm` supplies all its own exits (no trailing jmp is
        added). For fixes whose cave has more than one return target."""
        assert hook_len >= 5, "need 5 bytes for a near jmp at %x" % hook_va
        size = len(asm(cave_asm, CAVE_START))
        cave_va = self.alloc(size)
        full = asm(cave_asm, cave_va)
        assert len(full) == size, "cave length drift at %x" % cave_va
        patch = asm("jmp 0x%x\n" % cave_va, hook_va) + b"\x90" * (hook_len - 5)
        return (
            {"va": hook_va, "off": va_to_off(hook_va), "patched": patch.hex()},
            {"va": cave_va, "off": va_to_off(cave_va), "patched": full.hex(),
             "fill": "cc" if cave_va == CAVE_START else "00"},
        )

    def add(self, **fix):
        self.fixes.append(fix)


b = Builder()

# =============================================================================
# CLEAR BUGS (applied by default)
# =============================================================================

# --- mob-speed-id -----------------------------------------------------------
# CMob::SendSpeed (0x4906d0) builds the 0x0609 speed packet on the stack but never
# writes the monster's object id, so the 4 id bytes are whatever the stack held.
# Hook the 7 bytes at 0x490742 (add eax,0x50 ; mov [esp+7],cl); the cave writes the
# object id (m_dwID at CObject+0xc) into the packet before the send.
hook, cave = b.hook(
    0x490742, 7,
    "add eax,0x50\n"            # displaced: eax -> CObject base (center of the send)
    "mov edx,[eax+0xc]\n"       # m_dwID (object id)
    "mov [esp+2],edx\n"         # packet id field (was stack garbage)
    "mov [esp+7],cl\n",         # displaced: move-speed byte
    0x490749)
b.add(id="mob-speed-id", title="Monster speed packet carries the monster's id",
      category="clear", default=True, status="verified in disassembly",
      symptom="CMob::SendSpeed builds 0x0609 without the monster id; nearby players "
              "get stack garbage as the id when a monster resets its speed.",
      credit="NewShaiya analysis (docs/original/mob-fight.md section 5)",
      inplace=[hook, cave])

# --- gatekeeper-logout ------------------------------------------------------
# Gatekeeper teleport (CUser::PacketMaininterface) takes the fare even when a logout
# is pending and the move is skipped. The logout test at 0x4691a7 only skips the
# scheduling; the fare is subtracted afterwards at 0x4691e4. Hook the test+branch
# (9 bytes) so a pending logout jumps to the failure answer (0x46931f) before any
# money is taken, and otherwise runs the original scheduling at 0x4691b0.
hook, cave = b.hook(
    0x4691a7, 9,
    "cmp dword ptr [ebp+0x57dc],0\n"
    "jne 0x46931f\n",           # logout pending -> fail answer, no fare taken
    0x4691b0)                    # otherwise -> original scheduling
b.add(id="gatekeeper-logout", title="Gatekeeper takes no fare when a logout is pending",
      category="clear", default=True, status="verified in disassembly",
      symptom="With a logout pending the gatekeeper subtracts the fare and the player "
              "does not move.",
      credit="NewShaiya analysis (docs/original/gatekeeper.md section 2)",
      inplace=[hook, cave])

# --- time-damage-player -----------------------------------------------------
# CUser::TimeDamage (0x486290) computes base + elapsed/3000 for a damage-over-time
# tick on a player, where the monster version multiplies (base * elapsed/3000). Flip
# the three `fadd qword [esp+0x10]` (DC /0) to `fmul qword [esp+0x10]` (DC /1): one
# modrm byte each (0x44 -> 0x4c).
b.add(id="time-damage-player", title="Damage over time on players multiplies, as on monsters",
      category="clear", default=True, status="verified in disassembly",
      symptom="A damage-over-time debuff on a player adds elapsed/3000 to the base "
              "instead of multiplying, unlike the monster version.",
      credit="NewShaiya analysis (docs/original/mob-skills.md section 8)",
      inplace=[b.inplace(0x4864f6, "442410", "4c2410"),
               b.inplace(0x486527, "442410", "4c2410"),
               b.inplace(0x486559, "442410", "4c2410")])

# --- illusion-save ----------------------------------------------------------
# CUser::SendDBAgentCharGetInfo (0x46c980) saves every kind-4 skill except change
# type 100 (Disguise); Illusion (change type 102) is saved, so after a relog the
# player keeps shape 102 with no monster and loses item attack/defence. Hook the
# `cmp word [ebx+0x98],100 ; je skip` (14 bytes at 0x46cbc2) to also skip 102.
hook, cave = b.hook(
    0x46cbc2, 14,
    "cmp word ptr [ebx+0x98],100\n"
    "je 0x46cc7f\n"             # Disguise: not saved (original behaviour)
    "cmp word ptr [ebx+0x98],102\n"
    "je 0x46cc7f\n",            # Illusion: not saved either (the fix)
    0x46cbd0)
b.add(id="illusion-save", title="Illusion is not saved across a relog",
      category="clear", default=True, status="verified in disassembly",
      symptom="Illusion (change type 102) is saved with the character but its copied "
              "monster is not; after login the player has shape 102 with monster 0 and "
              "item attack/defence drop to 0 until it ends.",
      credit="NewShaiya analysis (docs/original/skills-special.md section 5.4)",
      inplace=[hook, cave])

# --- myshop-visit-cleanup ---------------------------------------------------
# When a player who is visiting a personal shop leaves the zone, CUser::LeaveZone
# (0x44aefa) calls stMyShop::Unuse on the *leaver's own* shop instead of the seller's,
# so the seller's visitor list keeps the leaver; 16 such stale entries fill the list.
# At the hook eax = the seller (from FindUser) and edi = the leaver's id; point eax at
# the seller's embedded stMyShop (+0x15dc) before the call.
hook, cave = b.hook(
    0x44af2e, 7,                # mov eax,ebp ; call stMyShop::Unuse
    "lea eax,[eax+0x15dc]\n"    # eax -> seller.m_MyShop (was the leaver's own shop)
    "call 0x4784a0\n",          # stMyShop::Unuse(seller shop, leaver id in edi)
    0x44af35)
b.add(id="myshop-visit-cleanup", title="Leaving a personal-shop visit clears the seller's list",
      category="clear", default=True, status="verified in disassembly",
      symptom="Leaving the zone while visiting a personal shop removes the visitor from "
              "its own shop, not the seller's, so the seller's list keeps dead visitors "
              "and fills up.",
      credit="NewShaiya analysis (docs/original/personal-shop.md section 6)",
      inplace=[hook, cave])

# --- learn-skill-level ------------------------------------------------------
# CUser::LearnSkill (0x4842e0) accepts any level above the one known, so a modified
# client can buy level 3 from level 1 for level 3's points, and a passive then has
# level 2's effect removed though it was never added. Require exactly current+1.
# Hook the known-skill test (cmp [ecx+0x10],ebx ; mov [esp+0x10],esi ; jl learn).
hook, cave = b.hook_raw(
    0x4844b0, 9,
    "mov [esp+0x10],esi\n"          # displaced: remember the skill slot
    "mov eax,[ecx+0x10]\n"          # current level
    "inc eax\n"
    "cmp eax,ebx\n"                 # current+1 == requested?
    "jne 0x4844b9\n"                # no -> result 3 (cannot learn)
    "jmp 0x4844e2\n")               # yes -> learn the next level
b.add(id="learn-skill-level", title="Only the next skill level can be learnt",
      category="clear", default=True, status="verified in disassembly",
      symptom="0x0209 accepts any level above the one known, so a modified client "
              "jumps skill levels; a passive then drifts for good.",
      credit="NewShaiya analysis (docs/original/skills.md sections 3 and 7)",
      inplace=[hook, cave])

# --- enchant-material-bounds ------------------------------------------------
# CUser::ItemLapisianAdd (0x45fb20) reads the enchant material's bag and slot and
# indexes the item table with no bounds check, so a bad place reads outside it.
# Bounds-check the material place; an out-of-range place yields no material.
hook, cave = b.hook_raw(
    0x45fbb3, 0x15,
    "movzx eax,byte ptr [ebp+4]\n"   # material bag
    "cmp eax,[edi+0x1304]\n"         # bag > m_nMaxBag ?
    "ja 1f\n"
    "movzx ecx,byte ptr [ebp+5]\n"   # material slot
    "cmp ecx,0x18\n"                 # slot >= 24 ?
    "jae 1f\n"
    "lea edx,[eax+eax*2]\n"          # bag*3
    "lea ecx,[ecx+edx*8]\n"          # slot + bag*24
    "mov ecx,[edi+ecx*4+0x1c4]\n"    # material item pointer
    "jmp 2f\n"
    "1: xor ecx,ecx\n"               # out of range -> no material
    "2: jmp 0x45fbc8\n")
b.add(id="enchant-material-bounds", title="The enchant material's place is bounds-checked",
      category="clear", default=True, status="verified in disassembly",
      symptom="0x0805's material bag and slot index the item table without bounds, so "
              "a bad place reads outside it (a crash, or a stray object as material).",
      credit="NewShaiya analysis (docs/original/lapis.md section 5)",
      inplace=[hook, cave])

# --- cloak-lapis ------------------------------------------------------------
# For a cloak (real type 24), CUser::EnableGemAdd (0x45fa50) and EnableLapisianAdd
# (0x45f960) answer yes before looking at the second item, so any item can be
# "linked" into a cloak or "enchant" it. Make a cloak take neither: route the cloak
# case to the function's "no" exit instead of its early "yes".
hook, cave = b.hook_raw(
    0x45fa69, 5,                      # cmp ebp,0x18 ; jne 0x45fa73
    "cmp ebp,0x18\n"
    "je 0x45fb17\n"                  # cloak -> "no" (was the early "yes")
    "jmp 0x45fa73\n")                # other -> the normal lapis-fit check
b.add(id="cloak-lapis", title="Cloaks take no lapis (link)",
      category="clear", default=True, status="verified in disassembly",
      symptom="EnableGemAdd says yes for a cloak before checking the lapis, so any item "
              "can be linked into a cloak for the price of its grade (usually free).",
      credit="NewShaiya analysis (docs/original/lapis.md sections 3 and 5)",
      inplace=[hook, cave])
hook, cave = b.hook_raw(
    0x45f979, 5,                      # cmp edi,0x18 ; jne 0x45f983  (cloak early-yes)
    "cmp edi,0x18\n"
    "je 0x45fa45\n"                  # cloak -> "no"
    "jmp 0x45f983\n")                # other -> the normal Lapisia-fit check
b.add(id="cloak-lapisia", title="Cloaks take no Lapisia (enchant)",
      category="clear", default=True, status="verified in disassembly",
      symptom="EnableLapisianAdd says yes for a cloak before checking the Lapisia, so "
              "any item can enchant a cloak (an apple to +20 absorption, free).",
      credit="NewShaiya analysis (docs/original/lapis.md sections 3 and 5)",
      inplace=[hook, cave])

# --- nan-pos ----------------------------------------------------------------
# CMap::IsSafePos (0x415360) checks only 0 <= x,z < size with x87 compares that let a
# NaN coordinate through. Reject NaN (x != x) at the top before the range test.
hook, cave = b.hook(
    0x415360, 10,                     # fld [esp+8] ; fcomp [0x53a428]
    "fld dword ptr [esp+8]\n"         # x
    "fucomp st(0)\n"                  # x vs x -> unordered if NaN
    "fnstsw ax\n"
    "test ah,0x44\n"
    "jnz 9f\n"
    "fld dword ptr [esp+0xc]\n"       # z
    "fucomp st(0)\n"
    "fnstsw ax\n"
    "test ah,0x44\n"
    "jnz 9f\n"
    "fld dword ptr [esp+8]\n"         # displaced: fld x
    "fcomp dword ptr ds:0x53a428\n"   # displaced: compare with 0.0
    "jmp 8f\n"
    "9: xor al,al\n"                  # NaN -> not safe
    "ret 0xc\n"
    "8: nop\n", 0x41536a)
b.add(id="nan-pos", title="NaN positions are refused",
      category="clear", default=True, status="verified in disassembly",
      symptom="CMap::IsSafePos lets NaN coordinates pass its x87 range checks.",
      credit="NewShaiya analysis (docs/original/maps-npcs.md section 2)",
      inplace=[hook, cave])

# --- bound-trade ------------------------------------------------------------
# CUser::PacketExchange (0x46ec90) never reads byOG when an item is offered in a
# trade, so a modified client can trade bound items. Add the byOG check next to the
# existing real-type-27 check.
hook, cave = b.hook(
    0x46f03a, 16,
    "mov esi,[eax+0x30]\n"           # stItemInfo
    "cmp dword ptr [esi+0xa0],0x1b\n"
    "je 0x46f2da\n"                  # quest item -> ignore (original)
    "cmp byte ptr [esi+0x31],0\n"    # byOG (bound) != 0 ?
    "jne 0x46f2da\n", 0x46f04a)      # bound -> ignore (the fix)
b.add(id="bound-trade", title="Bound items cannot be traded",
      category="clear", default=True, status="verified in disassembly",
      symptom="PacketExchange checks only real type 27, not byOG, so a modified client "
              "can trade account/character-bound items.",
      credit="NewShaiya analysis (docs/original/trade.md section 2.3)",
      inplace=[hook, cave])

# --- bound-duel -------------------------------------------------------------
# The duel stakes window (CUser::PacketPVP ~0x47d764) has the same gap as trade.
hook, cave = b.hook(
    0x47d764, 16,
    "mov esi,[eax+0x30]\n"
    "cmp dword ptr [esi+0xa0],0x1b\n"
    "je 0x47da19\n"
    "cmp byte ptr [esi+0x31],0\n"
    "jne 0x47da19\n", 0x47d774)
b.add(id="bound-duel", title="Bound items cannot be staked in a duel",
      category="clear", default=True, status="verified in disassembly",
      symptom="The duel stakes window checks only real type 27, not byOG, as the trade "
              "window does.",
      credit="NewShaiya analysis (docs/original/duel.md section 4)",
      inplace=[hook, cave])

# =============================================================================
# DESIGN-CHOICE FIXES (optional; applied only when named)
# =============================================================================

# --- fire-thorns-aggro ------------------------------------------------------
# CUser::BuffUser (0x450ac0) only adds a self-buff's aggro (Fire Thorns 47, Mass
# Taunt 105) when the caster is not in the attacking state, but an instant skill
# always goes off in that state, so the aggro never happens. Remove the status-2
# skip (NOP the `je` after the status compare) so the aggro reaches the caster's
# target monster. Design choice: it changes how these skills behave.
b.add(id="fire-thorns-aggro", title="Fire Thorns / Mass Taunt aggro works",
      category="design", default=False, status="verified in disassembly",
      symptom="A self buff with an aggro value should anger the monster the caster is "
              "fighting, but the server only does it when the caster is not attacking, "
              "which an instant skill never is.",
      credit="NewShaiya analysis (docs/original/skills-buffs.md section 4)",
      inplace=[b.inplace(0x450ca5, "7432", "9090")])

# --- op-exclusive -----------------------------------------------------------
# CUser::ItemGemAdd (0x461310) caps the link rate at 50% after a hammer multiplies
# it, so the Operator's Exclusive (2000%) is no sure link. Make a rate of 1000% or
# more certain. Design choice (deviation 4).
hook, cave = b.hook(
    0x4616fa, 10,                     # cmp ebx,0x32 ; jle done ; mov ebx,0x32
    "cmp ebx,0x3e8\n"                 # rate >= 1000% ?
    "jl 1f\n"
    "mov ebx,0x64\n"                  # yes -> 100 (rand()%100 < 100 always true)
    "jmp 2f\n"
    "1: cmp ebx,0x32\n"              # else -> the original 1..50 cap
    "jle 2f\n"
    "mov ebx,0x32\n"
    "2: nop\n", 0x461704)
b.add(id="op-exclusive", title="Hammers of 1000% or more always link",
      category="design", default=False, status="verified in disassembly",
      symptom="The staff Linking Hammer multiplies the rate like any hammer and the "
              "50% cap then holds it at 50%, though it was meant to be certain.",
      credit="NewShaiya analysis (docs/original/lapis.md section 3)",
      inplace=[hook, cave])

# --- party-exp-levelup ------------------------------------------------------
# CUser::AddExpFromUser (0x458240) adds the full-party (7 member) bonus only on a
# gain that does not level the player up; a gain that levels up drops it. Add the
# bonus before the level-up test so it is paid on every gain.
hook, cave = b.hook(
    0x45835f, 6,                      # cmp esi,eax ; jae levelup ; add esi,ebx
    "add esi,ebx\n"                   # the full-party bonus, once, before the test
    "cmp esi,eax\n"
    "jae 8f\n"                        # level up
    "jmp 0x458365\n"                  # no level up: resume past the original add
    "8: nop\n", 0x4583a0)
b.add(id="party-exp-levelup", title="The full-party experience bonus is paid on a level-up",
      category="clear", default=True, status="verified in disassembly",
      symptom="In a party of 7 each gain counts the bonus twice except a gain that "
              "levels the player up, which counts it once.",
      credit="NewShaiya analysis (docs/original/party.md section 8)",
      inplace=[hook, cave])

# --- stealth-equip ----------------------------------------------------------
# While the shape is not 0, CUser::ItemEquipmentAdd/Rem and GemEquipmentAdd/Rem skip
# an item's four fighting numbers. Stealth (hiding) sets the shape to 13 but, unlike
# a transformation, never recomputes on its end, so an item taken off and put back on
# in Stealth drifts the numbers. Fix: count items normally in Stealth (shape 13) while
# keeping the skip for real transformations. Each of the ten sites tests
# `cmp [reg+0x1354],0 ; jne skip`; replace the compare with a call to a helper that
# sets ZF for "shape in {0,13}", so the following jne skips only for other shapes.
def _stealth():
    helper_edi_asm = ("cmp byte ptr [edi+0x1354],0\n" "je 8f\n"
                      "cmp byte ptr [edi+0x1354],13\n" "8: ret\n")
    helper_eax_asm = ("cmp byte ptr [eax+0x1354],0\n" "je 8f\n"
                      "cmp byte ptr [eax+0x1354],13\n" "8: ret\n")
    hedi = b.alloc(len(asm(helper_edi_asm, CAVE_START)))
    hedi_bytes = asm(helper_edi_asm, hedi)
    heax = b.alloc(len(asm(helper_eax_asm, CAVE_START)))
    heax_bytes = asm(helper_eax_asm, heax)
    patches = [
        {"va": hedi, "off": va_to_off(hedi), "original": "00" * len(hedi_bytes),
         "patched": hedi_bytes.hex(), "fill": "00"},
        {"va": heax, "off": va_to_off(heax), "original": "00" * len(heax_bytes),
         "patched": heax_bytes.hex(), "fill": "00"},
    ]
    edi_sites = [0x454d1d, 0x454e21, 0x454fcb, 0x455200,
                 0x45544c, 0x455565, 0x4556ca, 0x455aeb]
    eax_sites = [0x456136, 0x4562f6]
    for site in edi_sites:
        patch = asm("call 0x%x\n" % hedi, site) + b"\x90\x90"
        patches.append({"va": site, "off": va_to_off(site),
                        "original": "80bf5413000000", "patched": patch.hex()})
    for site in eax_sites:
        patch = asm("call 0x%x\n" % heax, site) + b"\x90\x90"
        patches.append({"va": site, "off": va_to_off(site),
                        "original": "80b85413000000", "patched": patch.hex()})
    return patches


b.add(id="stealth-equip", title="Equipment counts normally in Stealth",
      category="clear", default=True, status="verified in disassembly",
      symptom="An item moved while hidden (Stealth, shape 13) keeps or doubles its "
              "attack and defence, because the equip code skips its numbers while any "
              "shape is set and Stealth never recomputes them.",
      credit="NewShaiya analysis (docs/original/skills-buffs.md section 5); the "
             "community's own fix is a code cave by Bowie (kreon800/shaiya) that skips "
             "item moves in shape 13, and an earlier ItemBagToBag cave of unnamed "
             "authorship; this kit instead counts the items.",
      inplace=_stealth())

# --- bound-shop -------------------------------------------------------------
# CUser::PacketMyShop place (0x2305, ~0x478e2b) stores a bag/slot/price without ever
# looking at the item, so a modified client can put a bound or quest item up for sale.
# At the store (0x478e9d: mov [ebp+edi*4+0x15e4],ecx) ecx = bag, ebx = slot, ebp =
# the user; fetch the item and refuse a quest (real type 27) or bound (byOG) one.
hook, cave = b.hook_raw(
    0x478e9d, 7,
    "push eax\n"
    "mov eax,ecx\n"                  # bag
    "lea eax,[eax+eax*2]\n"          # bag*3
    "movzx edx,bl\n"                 # slot
    "lea eax,[edx+eax*8]\n"          # slot + bag*24
    "mov eax,[ebp+eax*4+0x1c4]\n"    # item pointer
    "test eax,eax\n"
    "je 1f\n"                        # empty -> refuse
    "mov eax,[eax+0x30]\n"           # stItemInfo
    "cmp dword ptr [eax+0xa0],0x1b\n"
    "je 1f\n"                        # quest item -> refuse
    "cmp byte ptr [eax+0x31],0\n"
    "jne 1f\n"                       # bound -> refuse
    "pop eax\n"
    "mov [ebp+edi*4+0x15e4],ecx\n"   # displaced store
    "jmp 0x478ea4\n"
    "1: pop eax\n"
    "jmp 0x4791bd\n")                # refuse: the handler's silent-ignore exit
b.add(id="bound-shop", title="Bound items cannot be sold in a personal shop",
      category="clear", default=True, status="verified in disassembly",
      symptom="The personal-shop place handler stores a slot without checking the item, "
              "so a modified client can sell quest or bound items.",
      credit="NewShaiya analysis (docs/original/personal-shop.md sections 3.3 and 5)",
      inplace=[hook, cave])

# --- input filtering --------------------------------------------------------
# Append new caves so existing fixes retain their distributed bytes/addresses.
def filter_quotes(pointer, limit):
    """Normalize 0x27 to 0x20 until NUL or limit; preserve registers and EFLAGS.

    Callers already force a final NUL. Pointers use entry registers, never saved
    stack offsets. This is legacy byte normalization, not general SQL escaping.
    """
    return (
        "pushfd\npushad\n"
        "lea esi,%s\nmov ecx,%d\n" % (pointer, limit) +
        "1: cmp byte ptr [esi],0\nje 3f\n"
        "cmp byte ptr [esi],0x27\njne 2f\n"
        "mov byte ptr [esi],0x20\n"
        "2: inc esi\nloop 1b\n"
        "3: popad\npopfd\n")


# TrimString forces name[20]=NUL and removes spaces. Filter AFTER it so the
# replacement spaces survive, preserving the displaced call's output state.
name_patches = []
for site, resume in ((0x46c006, 0x46c00b), (0x46c173, 0x46c178)):
    name_patches.extend(b.hook(
        site, 5, "call 0x46c250\n" + filter_quotes("[ebp]", 20), resume))
b.add(id="character-name-filter", title="Character create and rename names replace quotes with spaces",
      category="clear", default=True, status="verified in disassembly",
      symptom="The database agent inserts create and rename names directly into "
              "Usp_Create_Char_R and usp_Save_Char_Name_E SQL text.",
      credit="NewShaiya analysis (docs/original/ps_game.md; docs/original/server-patch-notes.md)",
      inplace=name_patches)

# Guild create has just forced name[24] and remark[64] to NUL. Normalize before
# CGuildLock / CGuildMgr::CreateGuild so lookup, storage and sends agree.
guild_patches = list(b.hook(
    0x476507, 5,
    filter_quotes("[ebx+2]", 24) + filter_quotes("[ebx+0x1b]", 64) +
    "call 0x42b800\n", 0x47650c))
# Remark update forces request[0x42]=NUL and rejects quotes afterwards. Normalize
# before that check, so the sanitized remark is accepted rather than rejected.
guild_patches.extend(b.hook(
    0x477102, 5,
    "add ebx,2\nmov ecx,ebx\n" + filter_quotes("[ebx]", 64), 0x477107))
b.add(id="guild-text-filter", title="Guild names and remarks replace quotes with spaces",
      category="clear", default=True, status="verified in disassembly",
      symptom="Guild create formats the name and remark into SQL text; remark updates "
              "reject a quote instead of normalizing it. Normalize before both paths.",
      credit="NewShaiya analysis (CUser::PacketGuild; docs/original/server-patch-notes.md)",
      inplace=guild_patches)

# EBP=request+0x17; request[0x2b] is already NUL. strncpy's three arguments are
# already pushed. Restore ESP before replaying the displaced 21-byte copy.
hook, cave = b.hook(
    0x479f4a, 5,
    filter_quotes("[ebp]", 20) + "call 0x504c60\n", 0x479f4f)
b.add(id="gift-target-filter", title="Gift target names replace quotes with spaces",
      category="clear", default=True, status="verified in disassembly",
      symptom="PacketPoint copies the gift target into the database request used in "
              "the gift item and notification SQL statements without escaping it.",
      credit="NewShaiya analysis (CUser::PacketPoint; docs/original/server-patch-notes.md)",
      inplace=[hook, cave])

# Filter the byte copied to SendLogAdmin's 0x0C03 detail field, keeping the source
# notice (also displayed), original copy loop/NUL and length computation intact.
# The original ADD/TEST after resume still set the loop flags.
hook, cave = b.hook(
    0x471ca8, 5,
    "pushfd\nmov cl,[eax]\ncmp cl,0x27\njne 1f\nmov cl,0x20\n"
    "1: mov [edx+eax],cl\npopfd\n", 0x471cad)
b.add(id="gm-notice-filter", title="GM notice log text replaces quotes with spaces",
      category="clear", default=True, status="verified in disassembly",
      symptom="SendLogAdmin copies notice/detail text into 0x0C03 and the game log "
              "formats it directly into usp_Insert_Action_Log_E SQL text.",
      credit="NewShaiya analysis (docs/original/game-log.md section 3.8)",
      inplace=[hook, cave])

# =============================================================================
# PASS 2 (appended, so every earlier fix keeps its distributed bytes/addresses)
# =============================================================================

# --- myshop-price-wrap ------------------------------------------------------
# CUser::PacketMyShop buy (0x230D) computes count * slot price with a 32-bit
# `imul`, so a large price wraps to a small one. Use an unsigned `mul` and answer
# result 2 ("Insufficient gold") when the product needs more than 32 bits; the
# refusal at 0x479116 builds its own packet and needs no register.
hook, cave = b.hook_raw(
    0x4790ed, 11,                     # movzx edx,cl ; imul edx,[ebx+eax*4+0x1644]
    "movzx ecx,cl\n"                  # count
    "mov eax,[ebx+eax*4+0x1644]\n"    # the shop slot's price
    "mul ecx\n"                       # edx:eax = price * count, CF if edx != 0
    "jc 0x479116\n"                   # too big -> result 2
    "mov edx,eax\n"                   # the product, where the original keeps it
    "jmp 0x4790f8\n")
b.add(id="myshop-price-wrap", title="Personal-shop prices that wrap past 2^32 are refused",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="Buying from a personal shop multiplies count by price in 32 bits; a large "
              "price wraps to a small one, so the item sells for almost nothing.",
      credit="NewShaiya analysis (docs/original/personal-shop.md section 5); the same "
             "refusal as ScreaM's GoldBugFix.CT for NPC shops",
      inplace=[hook, cave])

# --- npc-shop-price-wrap ----------------------------------------------------
# CUser::PacketShop buy (0x0702) has the same 32-bit product (0x46b55e). Refuse an
# overflowing product with the not-enough-money answer (result 1) at 0x46b57e, as
# GoldBugFix does in memory; that path reads ebp (the item definition, [esp+0x10])
# and the opcode word at [esp+0x20], set here as the original sets them first.
hook, cave = b.hook_raw(
    0x46b55a, 11,                     # movzx eax,[ebx+7] ; imul eax,[esi+0x80]
    "push edx\n"
    "movzx eax,byte ptr [ebx+7]\n"    # count
    "mul dword ptr [esi+0x80]\n"      # price * count, CF if it needs 64 bits
    "pop edx\n"                       # (pop keeps the flags)
    "jc 1f\n"
    "jmp 0x46b565\n"
    "1: mov ebp,[esp+0x10]\n"
    "mov word ptr [esp+0x20],0x702\n"
    "jmp 0x46b57e\n")                 # result 1, as for too little money
b.add(id="npc-shop-price-wrap", title="NPC-shop prices that wrap past 2^32 are refused",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="Buying from an NPC multiplies count by price in 32 bits; a large count "
              "times a large price wraps to a small one.",
      credit="NewShaiya analysis (docs/original/shop.md); ScreaM's GoldBugFix.CT makes "
             "the same refusal in the running process (Cheat Engine)",
      inplace=[hook, cave])

# --- remake-distinct --------------------------------------------------------
# CUser::ItemRemake (0x0807) never checks that its three places differ, so one item
# named three times becomes a new item and its object is freed three times. Ignore
# the request (the handler's silent exit 0x461013) when two of the three item
# pointers are the same, before the first make-type test.
hook, cave = b.hook(
    0x460983, 6,                      # mov bl,[esi+0x68] ; cmp bl,0x51
    "cmp esi,ecx\n" "je 0x461013\n"
    "cmp esi,edx\n" "je 0x461013\n"
    "cmp ecx,edx\n" "je 0x461013\n"
    "mov bl,[esi+0x68]\n"
    "cmp bl,0x51\n", 0x460989)        # the original je on these flags
b.add(id="remake-distinct", title="Remaking needs three different items",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="A remake request may name the same place three times: one item becomes "
              "a new one with fresh craft stats, and its object is freed three times.",
      credit="NewShaiya analysis (docs/original/lapis.md section 7)",
      inplace=[hook, cave])

# --- enchant-seed -----------------------------------------------------------
# CUser::ItemLapisianAdd builds a fresh SRandom on the stack for every enchant and
# seeds it with time(0) xor the session id, so two enchants of one player in the
# same second roll the same number. (Nothing else seeds it; without the call the
# constructor would seed with time(0) alone.) Mix the CPU cycle counter in.
hook, cave = b.hook(
    0x45fc8e, 8,                      # mov edi,eax ; xor edi,[ebx+0x575c]
    "mov edi,eax\n"
    "xor edi,[ebx+0x575c]\n"
    "rdtsc\n"                         # eax, edx: neither is live here
    "xor edi,eax\n", 0x45fc96)
b.add(id="enchant-seed", title="Each enchant roll gets its own seed",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="The enchant roll is seeded with the time in seconds xor the session id, "
              "so two enchants within a second roll the same number.",
      credit="NewShaiya analysis (docs/original/lapis.md section 5)",
      inplace=[hook, cave])

# --- quest-end-npc ----------------------------------------------------------
# CUser::QuestEndRequest (0x0903) skips the end NPC check whenever the packet's NPC
# id is 0. On the branch for quests that name an end NPC, refuse NPC id 0 (the
# function's silent exit); quests without an end NPC are unchanged.
hook, cave = b.hook_raw(
    0x47f2fc, 5,                      # cmp eax,3 ; ja 0x47f358
    "cmp dword ptr [esp+0x18],0\n"    # the packet's NPC id
    "je 0x47f358\n"
    "cmp eax,3\n"
    "ja 0x47f358\n"
    "jmp 0x47f301\n")                 # the original jump table
b.add(id="quest-end-npc", title="Quests that name an end NPC are handed in at it",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="A quest end request with NPC id 0 skips the end NPC, so item, monster "
              "and player-kill quests can be handed in anywhere.",
      credit="NewShaiya analysis (docs/original/quest.md sections 7 and 11)",
      inplace=[hook, cave])

# --- chat-markup-filter -----------------------------------------------------
# game.exe renders `<<` + 3 digits + `c` + 2 digits as a colour/link marker for the
# next NNN bytes; one whose NNN runs past the text crashes every client showing it.
# One shared routine walks a text up to its first NUL (or a limit), keeps each
# well-formed marker with 1 <= NNN <= the bytes after it, and turns the first `<` of
# any other `<<` into a space. Byte count and NUL are unchanged; valid markers (the
# client's own /chatcolor and item names) pass. In: esi text, ecx limit; preserves
# every register and EFLAGS.
MARKUP_SCAN = (
    "pushfd\npushad\n"
    "mov edi,esi\n"
    "test ecx,ecx\njz 5f\n"
    "1: cmp byte ptr [edi],0\nje 3f\ninc edi\nloop 1b\n"   # edi = the text's end
    "3: lea eax,[esi+1]\ncmp eax,edi\njae 5f\n"            # fewer than 2 left
    "cmp word ptr [esi],0x3c3c\njne 4f\n"                  # "<<"
    "lea eax,[esi+8]\ncmp eax,edi\nja 6f\n"                # a whole marker?
    "xor ebx,ebx\nmov edx,2\n"
    "7: movzx eax,byte ptr [esi+edx]\nsub eax,0x30\ncmp eax,9\nja 6f\n"
    "imul ebx,ebx,10\nadd ebx,eax\ninc edx\ncmp edx,5\njb 7b\n"  # NNN
    "cmp byte ptr [esi+5],0x63\njne 6f\n"                  # 'c'
    "movzx eax,byte ptr [esi+6]\nsub eax,0x30\ncmp eax,9\nja 6f\n"
    "movzx eax,byte ptr [esi+7]\nsub eax,0x30\ncmp eax,9\nja 6f\n"
    "test ebx,ebx\njz 6f\n"
    "mov eax,edi\nsub eax,esi\nsub eax,8\ncmp ebx,eax\nja 6f\n"  # NNN fits
    "add esi,8\njmp 3b\n"
    "6: mov byte ptr [esi],0x20\n"                         # break this "<<"
    "4: inc esi\njmp 3b\n"
    "5: popad\npopfd\nret\n")


def _chat():
    scan = b.alloc(len(asm(MARKUP_SCAN, CAVE_START)))
    patches = [{"va": scan, "off": va_to_off(scan), "patched": asm(MARKUP_SCAN, scan).hex(),
                "fill": "00"}]
    # PacketChatE: every 11xx case is `u8 len, text` at +2 except the whisper 1102
    # (name[21], len at +0x17); len must be <= 0x80 or the case closes anyway.
    patches.extend(b.hook(
        0x47052f, 8,
        "push ecx\npush esi\n"
        "movzx eax,word ptr [esi]\n"
        "cmp eax,0x1102\njne 1f\n"
        "movzx ecx,byte ptr [esi+0x17]\nadd esi,0x18\njmp 2f\n"
        "1: sub eax,0x1101\ncmp eax,0x11\nja 3f\n"
        "mov ecx,0x300df\nbt ecx,eax\njnc 3f\n"      # 1101-1105, 1107, 1108, 1111, 1112
        "movzx ecx,byte ptr [esi+2]\nadd esi,3\n"
        "2: cmp ecx,0x80\nja 3f\n"
        "call 0x%x\n"
        "3: pop esi\npop ecx\n"
        "movzx eax,word ptr [esi]\nadd eax,0xffffeeff\n" % scan, 0x470537))
    # PacketAdminChat (packet in edi): F101/F103/F104/F105/F108/F10A as 11xx, F102
    # as the whisper, F106 (the GM's server-wide line) `u16 len, text` at +4 whose
    # last byte the handler forces to NUL, at most 0x7D0.
    patches.extend(b.hook(
        0x470f5a, 8,
        "push ecx\npush esi\nmov esi,edi\n"
        "movzx eax,word ptr [edi]\nsub eax,0xf101\ncmp eax,9\nja 3f\n"
        "cmp eax,1\njne 1f\n"
        "movzx ecx,byte ptr [esi+0x17]\nadd esi,0x18\njmp 2f\n"
        "1: cmp eax,5\njne 4f\n"
        "movzx ecx,word ptr [esi+2]\ncmp ecx,0x7d0\nja 3f\ncmp ecx,2\njb 3f\n"
        "dec ecx\nadd esi,4\ncall 0x%x\njmp 3f\n"
        "4: mov ecx,0x29d\nbt ecx,eax\njnc 3f\n"
        "movzx ecx,byte ptr [esi+2]\nadd esi,3\n"
        "2: cmp ecx,0x80\nja 3f\n"
        "call 0x%x\n"
        "3: pop esi\npop ecx\n"
        "movzx eax,word ptr [edi]\nadd eax,0xffff0eff\n" % (scan, scan), 0x470f62))
    # PacketAdminCmdD notices F906-F90B (packet in ebx): the length byte is at +2
    # (F906), +0x19 (F908), +5 (F90A) or +4 (F907, F909, F90B), the text after it.
    patches.extend(b.hook(
        0x4739d6, 8,
        "push ecx\npush esi\n"
        "movzx eax,word ptr [ebx]\nsub eax,0xf906\ncmp eax,5\nja 3f\n"
        "lea esi,[ebx+2]\ntest eax,eax\njz 2f\n"
        "lea esi,[ebx+0x19]\ncmp eax,2\nje 2f\n"
        "lea esi,[ebx+5]\ncmp eax,4\nje 2f\n"
        "lea esi,[ebx+4]\n"
        "2: movzx ecx,byte ptr [esi]\ninc esi\ncmp ecx,0x80\nja 3f\n"
        "call 0x%x\n"
        "3: pop esi\npop ecx\n"
        "movzx eax,word ptr [ebx]\nadd eax,0xffff06ff\n" % scan, 0x4739de))
    # PacketComm memos 0x2206 (friend) and 0x220B (block): the memo at +7 is copied
    # up to its first NUL, with byte 57 forced to NUL by the case: 50 bytes at most.
    patches.extend(b.hook_raw(
        0x4758a6, 7,
        "cmp eax,4\nje 1f\ncmp eax,9\njne 2f\n"
        "1: push ecx\npush esi\nlea esi,[ebx+7]\nmov ecx,50\ncall 0x%x\npop esi\npop ecx\n"
        "2: jmp dword ptr [eax*4+0x476280]\n" % scan))
    # PacketMyShop 0x2303 open: the title (shown to everyone around, and to players
    # arriving later) at +3, already cut at byte 120.
    patches.extend(b.hook(
        0x478d0a, 9,
        "lea eax,[esi+3]\npush esi\npush ecx\nmov esi,eax\nmov ecx,120\n"
        "call 0x%x\npop ecx\npop esi\n"
        "lea esi,[ebp+0x1678]\n" % scan, 0x478d13))
    return patches


b.add(id="chat-markup-filter", title="Malformed chat colour markers are broken up",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="A colour marker whose length runs past the text crashes every client "
              "that shows the line; the server relays it in chat, GM chat and notices, "
              "memos and shop titles.",
      credit="NewShaiya analysis (docs/client/client-packets.md, the chat markup crash)",
      inplace=_chat())

# --- guild-bank-bound -------------------------------------------------------
# CGuild::ItemGuildBankToBag onto a taken bag place swaps, putting the bag's item
# into the bank without the deposit's checks (real type 27, byOG). Apply them; a
# refused swap releases the guild's lock (held at [esp+0x2c]) and returns silently,
# as the deposit refuses.
hook, cave = b.hook_raw(
    0x42ffeb, 6,                      # je 0x430235 (bag place empty: plain move)
    "je 0x430235\n"
    "mov ecx,[esi]\n"                 # the bag's item
    "mov ecx,[ecx+0x30]\n"            # stItemInfo
    "cmp dword ptr [ecx+0xa0],0x1b\n" # quest item
    "je 1f\n"
    "cmp byte ptr [ecx+0x31],0\n"     # bound
    "jne 1f\n"
    "jmp 0x42fff1\n"                  # the original swap
    "1: push dword ptr [esp+0x2c]\n"
    "call dword ptr ds:0x5210d0\n"    # LeaveCriticalSection
    "jmp 0x430624\n")
b.add(id="guild-bank-bound", title="Bound items cannot enter the guild bank by a swap",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="Taking an item out of the guild bank onto a taken bag place puts the bag "
              "item into the bank unchecked, so a bound or quest item can pass to "
              "another member.",
      credit="NewShaiya analysis (docs/original/guild-bank.md section 13)",
      inplace=[hook, cave])

# --- gm-target-filter -------------------------------------------------------
# The two- and three-argument CUser::SendLogAdmin copy the GM's typed target name
# into 0x0C02/0x0C03, which ps_gameLog puts in quoted SQL text unescaped. Replace a
# quote with a space in the copy, as gm-notice-filter does for the detail.
GM_COPY = ("pushfd\nmov cl,[eax]\ncmp cl,0x27\njne 1f\nmov cl,0x20\n"
           "1: mov [edx+eax],cl\npopfd\n")
target_patches = []
for site in (0x471b94, 0x471c94):
    target_patches.extend(b.hook(site, 5, GM_COPY, site + 5))
b.add(id="gm-target-filter", title="GM log target names replace quotes with spaces",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="SendLogAdmin copies the GM's typed target name into 0x0C02/0x0C03 and the "
              "game log formats it directly into usp_Insert_Action_Log_E SQL text.",
      credit="NewShaiya analysis (docs/original/game-log.md section 3.8)",
      inplace=target_patches)

# --- exchange-count-guard ---------------------------------------------------
# CUser::ExchangeItemAdd (trade) and CUser::ExchangePvP (duel stakes) read each
# offered slot at the end; when the count offered differs from the count there,
# they make a new item of the offered count and subtract it from the giver's byte
# count without checking it is still there, so a count lowered after the offer
# wraps (3 - 5 = 254). Skip such a slot, as one whose item is gone.
trade_patches = list(b.hook_raw(
    0x46f9db, 12,                     # mov al,[edi+2] ; cmp al,[ebp+0x42] ; jne split
    "mov al,[edi+2]\ncmp al,[ebp+0x42]\n"
    "je 0x46f9e7\n"                   # all of it: the whole item moves
    "ja 0x4700e6\n"                   # more than is there now: skip the slot
    "jmp 0x46fb21\n"))                # part of it: split (original)
trade_patches.extend(b.hook_raw(
    0x47ccbb, 8,                      # mov cl,[ebx+2] ; cmp cl,[ebp+0x42] ; jne split
    "mov cl,[ebx+2]\ncmp cl,[ebp+0x42]\n"
    "je 0x47ccc3\n"
    "ja 0x47d052\n"
    "jmp 0x47cd0f\n"))
b.add(id="exchange-count-guard", title="A trade or duel stake that shrank is skipped",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="A trade or duel stake hands over the count offered and subtracts it from "
              "whatever is in the slot by then; a count lowered after the offer wraps "
              "round and duplicates the item.",
      credit="NewShaiya analysis (docs/original/trade.md sections 3 and 6, "
             "docs/original/duel.md section 4)",
      inplace=trade_patches)

# --- party-kick-leader ------------------------------------------------------
# CParty::ExpelUser (0x447120) moves the later members down and drops the count but,
# unlike CParty::LeaveNormalParty (0x446960), leaves the leader slot (+0xc) and the
# loot turn counter (+0x108) alone. Two hooks: a leader kicking itself is refused
# (the function's "not found" exit, nothing sent), and a kicked slot before the
# leader's (or before the loot turn) moves both down by one as leaving does.
kick_patches = list(b.hook(
    0x44718d, 6,                      # mov eax,[edi+0xdc]
    "cmp ebx,[ebp+0xc]\n"            # the kicked slot is the leader's: refuse
    "je 0x44716f\n"                  # the exit for a member that was not found
    "mov eax,[edi+0xdc]\n",
    0x447193))
kick_patches.extend(b.hook(
    0x4471d4, 7,                      # add [ebp+0x10],-1 ; mov eax,[ebp+0x10]
    "mov ecx,[ebp+0x108]\n"          # the loot turn counter
    "lea eax,[ebx+1]\n"
    "cmp ecx,eax\n"
    "jle .Lturn\n"
    "dec ecx\n"
    "mov [ebp+0x108],ecx\n"
    ".Lturn:\n"
    "mov eax,[ebp+0xc]\n"            # the leader's slot
    "cmp ebx,eax\n"
    "jge .Lleader\n"                 # kicked after the leader: unchanged
    "dec eax\n"
    "mov [ebp+0xc],eax\n"
    ".Lleader:\n"
    "add dword ptr [ebp+0x10],-1\n"  # displaced, last: its flags feed the jne
    "mov eax,[ebp+0x10]\n",
    0x4471db))
b.add(id="party-kick-leader", title="Kicking a party member keeps the leader and the loot turn",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="Kicking a member who came before the leader in the party silently makes "
              "the next member the leader on the server while every client still shows "
              "the old one; a leader kicking itself does the same.",
      credit="NewShaiya analysis (docs/original/party.md section 4)",
      inplace=kick_patches)

# --- repair-shield-slot -----------------------------------------------------
# CUser::ItemRepair (0x464340), one item: after setting the item's quality it also
# writes the new quality to the saved-quality copy (+0x1aa, u16 per slot) at the
# index left in edx by the gem loop (always 6, the shield). The copy belongs to the
# repaired slot, and only for the equipped slots (bag 0); a bag item has no copy.
repair_patches = list(b.hook(
    0x4649cf, 12,                     # mov [esi+0x44],di ; mov [ebp+edx*2+0x1aa],di
    "mov [esi+0x44],di\n"
    "test ebx,ebx\n"                 # ebx = bag
    "jne .Lbag\n"
    "mov edx,[esp+0xdc]\n"           # the repaired slot
    "mov [ebp+edx*2+0x1aa],di\n"
    ".Lbag:\n"
    "test ebx,ebx\n",                # the jne after the hook reads these flags
    0x4649db))
b.add(id="repair-shield-slot", title="Repairing one item updates that item's saved quality",
      category="clear", default=True, status="verified in disassembly and emulation",
      symptom="After repairing a single item, the server records the new quality for "
              "the shield's slot whatever was repaired, so the shield's own quality "
              "change may later go unsaved or be saved twice.",
      credit="NewShaiya analysis (docs/original/repair.md section 5)",
      inplace=repair_patches)

OUT = os.path.join(os.path.dirname(__file__), "fixes.json")


def main():
    exe = sys.argv[1] if len(sys.argv) > 1 else None
    data = None
    if exe:
        data = open(exe, "rb").read()
        if hashlib.sha256(data).hexdigest() != EXPECTED_SHA256:
            raise SystemExit("not the expected ps_game.exe build")
    problems = []
    for fix in b.fixes:
        for p in fix["inplace"]:
            if "original" in p and data is not None:
                off = p["off"]
                have = data[off:off + len(bytes.fromhex(p["patched"]))].hex()
                if have != p["original"]:
                    problems.append("%s @%x: expected %s, exe has %s"
                                    % (fix["id"], p["va"], p["original"], have))
            if "original" not in p and data is not None:
                # hook/cave: record what the pristine exe holds so the tool can verify.
                off = p["off"]
                p["original"] = data[off:off + len(bytes.fromhex(p["patched"]))].hex()
    if problems:
        print("\n".join(problems))
        raise SystemExit("original-byte mismatch; aborting")
    out = {
        "target": {"name": "ps_game.exe", "sha256": EXPECTED_SHA256, "size": SIZE,
                   "built": "2009-07-14",
                   "note": "EP4.5 game server. VA = file offset + 0x%x in .text." % IMAGE_BASE},
        "header": {"va": IMAGE_BASE + VSIZE_OFF, "off": VSIZE_OFF,
                   "original": VSIZE_ORIG.hex(), "patched": VSIZE_PATCHED.hex(),
                   "note": ".text VirtualSize, raised to map the code cave explicitly"},
        "cave": {"start": CAVE_START, "end": CAVE_END, "next_free": b.cave},
        "fixes": b.fixes,
    }
    json.dump(out, open(OUT, "w"), indent=2)
    print("wrote %s: %d fixes, cave used 0x%x..0x%x"
          % (OUT, len(b.fixes), CAVE_START, b.cave))


if __name__ == "__main__":
    main()
