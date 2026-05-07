# Migration: Scripts (content scripts directory)

> **C++ canonical path:** `src/server/scripts/` (the content-side script library: bosses, instances, quests, NPCs, gossip, spells, area triggers, GM commands, world events, holiday events, OutdoorPvP, Battlefield WG, pet AIs, custom scripts).
> **Rust target crate(s):** `crates/wow-scripts/` (content) + lateral crates (`wow-spell` for `spell_*` scripts, `wow-pvp`/`wow-battleground` for OutdoorPvP & Battlefield, `wow-pet` for Pet scripts, etc.). The framework that scripts plug into lives in `crates/wow-script/` and is covered by `scripting.md`.
> **Layer:** L8 (content layer — depends on **everything** below it: scripting framework L7, instance/BG/OPvP L7, spells L5, AI L5, conditions L7, loot L6, quests L6, all entity types L4, …).
> **Status:** ❌ not started — `crates/wow-scripts/src/lib.rs` is **0 bytes**. **No** boss AI, **no** instance script, **no** spell script, **no** GM command, **no** holiday event, **no** OutdoorPvP, **no** Wintergrasp, **no** quest helper exists in Rust. This is the largest single migration surface in the entire project: ~725 `.cpp` files and ~294,137 lines (it dwarfs every other subsystem in line count).
> **Audited vs C++:** ✅ audited 2026-05-01 (status confirmed ❌ — `wc -l` on `lib.rs` returns 0)
> **Last updated:** 2026-05-01

---

## 1. Purpose

`src/server/scripts/` holds the **content** layer of TrinityCore: everything that makes the game *be a game* rather than an inert simulation. Every boss encounter, every instance state machine, every WoLK quest helper NPC, every gossip flowchart, every holiday vendor swap, every `.gm fly` chat command, every Wintergrasp tower interaction, every special spell script (death-grip, kill command, polymorph trigger, etc.) is implemented here. Files derive from base classes defined in `src/server/game/Scripting/ScriptMgr.h` (covered by `scripting.md`); each file ends with an `AddSC_*` registration function. The aggregate `ScriptLoader.cpp.in.cmake` template wires every `AddSC_*` declaration into a single `AddScripts()` entry point that `ScriptMgr::Initialize()` invokes at startup.

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `scripts/Battlefield/BattlefieldWG.cpp` | 1857 | `prefix` |
| `scripts/Battlefield/BattlefieldWG.h` | 551 | `prefix` |
| `scripts/Battlefield/battlefield_script_loader.cpp` | 23 | `prefix` |
| `scripts/Custom/custom_script_loader.cpp` | 24 | `prefix` |
| `scripts/EasternKingdoms/AlteracValley/alterac_valley.cpp` | 179 | `prefix` |
| `scripts/EasternKingdoms/AlteracValley/boss_balinda.cpp` | 184 | `prefix` |
| `scripts/EasternKingdoms/AlteracValley/boss_drekthar.cpp` | 137 | `prefix` |
| `scripts/EasternKingdoms/AlteracValley/boss_galvangar.cpp` | 138 | `prefix` |
| `scripts/EasternKingdoms/AlteracValley/boss_vanndar.cpp` | 98 | `prefix` |
| `scripts/EasternKingdoms/ArathiBasin/arathi_basin.cpp` | 81 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/blackrock_depths.cpp` | 644 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/blackrock_depths.h` | 95 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_ambassador_flamelash.cpp` | 91 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_coren_direbrew.cpp` | 583 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_emperor_dagran_thaurissan.cpp` | 120 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_general_angerforge.cpp` | 132 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_high_interrogator_gerstahn.cpp` | 101 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_magmus.cpp` | 168 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_moira_bronzebeard.cpp` | 104 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/boss_tomb_of_seven.cpp` | 280 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockDepths/instance_blackrock_depths.cpp` | 442 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/blackrock_spire.h` | 134 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_drakkisath.cpp` | 102 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_gizrul_the_slavener.cpp` | 104 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_gyth.cpp` | 177 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_halycon.cpp` | 110 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_highlord_omokk.cpp` | 90 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_lord_valthalak.cpp` | 136 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_mother_smolderweb.cpp` | 95 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_overlord_wyrmthalak.cpp` | 135 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_pyroguard_emberseer.cpp` | 356 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_quartermaster_zigris.cpp` | 90 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_rend_blackhand.cpp` | 452 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_shadow_hunter_voshgajin.cpp` | 97 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_the_beast.cpp` | 107 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_urok_doomhowl.cpp` | 99 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/boss_warmaster_voone.cpp` | 116 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackrockSpire/instance_blackrock_spire.cpp` | 640 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/blackwing_lair.h` | 101 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_broodlord_lashlayer.cpp` | 219 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_chromaggus.cpp` | 316 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_ebonroc.cpp` | 86 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_firemaw.cpp` | 88 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_flamegor.cpp` | 94 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_nefarian.cpp` | 584 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_razorgore.cpp` | 201 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/boss_vaelastrasz.cpp` | 255 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/BlackwingLair/instance_blackwing_lair.cpp` | 316 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_baron_geddon.cpp` | 137 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_garr.cpp` | 157 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_gehennas.cpp` | 98 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_golemagg.cpp` | 167 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_lucifron.cpp` | 96 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_magmadar.cpp` | 110 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_majordomo_executus.cpp` | 204 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_ragnaros.cpp` | 326 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_shazzrah.cpp` | 149 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/boss_sulfuron_harbinger.cpp` | 195 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/instance_molten_core.cpp` | 213 | `prefix` |
| `scripts/EasternKingdoms/BlackrockMountain/MoltenCore/molten_core.h` | 86 | `prefix` |
| `scripts/EasternKingdoms/Deadmines/boss_mr_smite.cpp` | 226 | `prefix` |
| `scripts/EasternKingdoms/Deadmines/deadmines.cpp` | 22 | `prefix` |
| `scripts/EasternKingdoms/Deadmines/deadmines.h` | 61 | `prefix` |
| `scripts/EasternKingdoms/Deadmines/instance_deadmines.cpp` | 238 | `prefix` |
| `scripts/EasternKingdoms/Gnomeregan/gnomeregan.cpp` | 547 | `prefix` |
| `scripts/EasternKingdoms/Gnomeregan/gnomeregan.h` | 69 | `prefix` |
| `scripts/EasternKingdoms/Gnomeregan/instance_gnomeregan.cpp` | 120 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_curator.cpp` | 190 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_maiden_of_virtue.cpp` | 131 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_midnight.cpp` | 388 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_moroes.cpp` | 817 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_netherspite.cpp` | 355 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_nightbane.cpp` | 455 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_prince_malchezaar.cpp` | 583 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_shade_of_aran.cpp` | 607 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/boss_terestian_illhoof.cpp` | 323 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/bosses_opera.cpp` | 1543 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/instance_karazhan.cpp` | 356 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/karazhan.cpp` | 629 | `prefix` |
| `scripts/EasternKingdoms/Karazhan/karazhan.h` | 121 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/boss_felblood_kaelthas.cpp` | 510 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/boss_priestess_delrissa.cpp` | 1322 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/boss_selin_fireheart.cpp` | 285 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/boss_vexallus.cpp` | 220 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/instance_magisters_terrace.cpp` | 236 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/magisters_terrace.cpp` | 117 | `prefix` |
| `scripts/EasternKingdoms/MagistersTerrace/magisters_terrace.h` | 107 | `prefix` |
| `scripts/EasternKingdoms/ScarletEnclave/chapter1.cpp` | 1328 | `prefix` |
| `scripts/EasternKingdoms/ScarletEnclave/chapter2.cpp` | 632 | `prefix` |
| `scripts/EasternKingdoms/ScarletEnclave/chapter5.cpp` | 1666 | `prefix` |
| `scripts/EasternKingdoms/ScarletEnclave/zone_the_scarlet_enclave.cpp` | 143 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_arcanist_doan.cpp` | 119 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_azshir_the_sleepless.cpp` | 108 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_bloodmage_thalnos.cpp` | 115 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_headless_horseman.cpp` | 1041 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_herod.cpp` | 162 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_high_inquisitor_fairbanks.cpp` | 161 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_houndmaster_loksey.cpp` | 72 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_interrogator_vishas.cpp` | 118 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_mograine_and_whitemane.cpp` | 435 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/boss_scorn.cpp` | 80 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/instance_scarlet_monastery.cpp` | 175 | `prefix` |
| `scripts/EasternKingdoms/ScarletMonastery/scarlet_monastery.h` | 103 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_darkmaster_gandling.cpp` | 376 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_death_knight_darkreaver.cpp` | 63 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_doctor_theolen_krastinov.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_illucia_barov.cpp` | 112 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_instructor_malicia.cpp` | 156 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_jandice_barov.cpp` | 117 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_kirtonos_the_herald.cpp` | 307 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_kormok.cpp` | 214 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_lord_alexei_barov.cpp` | 106 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_lorekeeper_polkelt.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_ras_frostwhisper.cpp` | 125 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_the_ravenian.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/boss_vectus.cpp` | 139 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/instance_scholomance.cpp` | 214 | `prefix` |
| `scripts/EasternKingdoms/Scholomance/scholomance.h` | 72 | `prefix` |
| `scripts/EasternKingdoms/ShadowfangKeep/boss_apothecary_hummel.cpp` | 487 | `prefix` |
| `scripts/EasternKingdoms/ShadowfangKeep/instance_shadowfang_keep.cpp` | 246 | `prefix` |
| `scripts/EasternKingdoms/ShadowfangKeep/shadowfang_keep.cpp` | 365 | `prefix` |
| `scripts/EasternKingdoms/ShadowfangKeep/shadowfang_keep.h` | 44 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_baron_rivendare.cpp` | 147 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_baroness_anastari.cpp` | 136 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_cannon_master_willey.cpp` | 235 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_dathrohan_balnazzar.cpp` | 227 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_magistrate_barthilas.cpp` | 138 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_maleki_the_pallid.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_nerubenkan.cpp` | 121 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_order_of_silver_hand.cpp` | 169 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_postmaster_malown.cpp` | 138 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_ramstein_the_gorger.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/boss_timmy_the_cruel.cpp` | 105 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/instance_stratholme.cpp` | 541 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/stratholme.cpp` | 398 | `prefix` |
| `scripts/EasternKingdoms/Stratholme/stratholme.h` | 135 | `prefix` |
| `scripts/EasternKingdoms/SunkenTemple/instance_sunken_temple.cpp` | 231 | `prefix` |
| `scripts/EasternKingdoms/SunkenTemple/sunken_temple.cpp` | 151 | `prefix` |
| `scripts/EasternKingdoms/SunkenTemple/sunken_temple.h` | 61 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_brutallus.cpp` | 367 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_eredar_twins.cpp` | 745 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_felmyst.cpp` | 552 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_kalecgos.cpp` | 793 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_kiljaeden.cpp` | 1466 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/boss_muru.cpp` | 633 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/instance_sunwell_plateau.cpp` | 140 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/sunwell_plateau.cpp` | 63 | `prefix` |
| `scripts/EasternKingdoms/SunwellPlateau/sunwell_plateau.h` | 126 | `prefix` |
| `scripts/EasternKingdoms/TheStockade/boss_hogger.cpp` | 170 | `prefix` |
| `scripts/EasternKingdoms/TheStockade/boss_lord_overheat.cpp` | 91 | `prefix` |
| `scripts/EasternKingdoms/TheStockade/boss_randolph_moloch.cpp` | 193 | `prefix` |
| `scripts/EasternKingdoms/TheStockade/instance_the_stockade.cpp` | 53 | `prefix` |
| `scripts/EasternKingdoms/TheStockade/the_stockade.h` | 52 | `prefix` |
| `scripts/EasternKingdoms/Uldaman/boss_archaedas.cpp` | 426 | `prefix` |
| `scripts/EasternKingdoms/Uldaman/boss_ironaya.cpp` | 99 | `prefix` |
| `scripts/EasternKingdoms/Uldaman/instance_uldaman.cpp` | 521 | `prefix` |
| `scripts/EasternKingdoms/Uldaman/uldaman.cpp` | 66 | `prefix` |
| `scripts/EasternKingdoms/Uldaman/uldaman.h` | 75 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_akilzon.cpp` | 96 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_daakara.cpp` | 111 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_halazzi.cpp` | 104 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_hexlord.cpp` | 140 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_janalai.cpp` | 105 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/boss_nalorakk.cpp` | 110 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/instance_zulaman.cpp` | 304 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/zulaman.cpp` | 229 | `prefix` |
| `scripts/EasternKingdoms/ZulAman/zulaman.h` | 89 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_grilek.cpp` | 81 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_hazzarah.cpp` | 81 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_jindo_the_godbreaker.cpp` | 105 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_kilnara.cpp` | 92 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_mandokir.cpp` | 631 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_renataki.cpp` | 78 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_venoxis.cpp` | 97 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_wushoolay.cpp` | 73 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/boss_zanzil.cpp` | 106 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/instance_zulgurub.cpp` | 195 | `prefix` |
| `scripts/EasternKingdoms/ZulGurub/zulgurub.h` | 101 | `prefix` |
| `scripts/EasternKingdoms/eastern_kingdoms_script_loader.cpp` | 378 | `prefix` |
| `scripts/EasternKingdoms/zone_blasted_lands.cpp` | 90 | `prefix` |
| `scripts/EasternKingdoms/zone_burning_steppes.cpp` | 21 | `prefix` |
| `scripts/EasternKingdoms/zone_dun_morogh.cpp` | 89 | `prefix` |
| `scripts/EasternKingdoms/zone_dun_morogh_area_coldridge_valley.cpp` | 467 | `prefix` |
| `scripts/EasternKingdoms/zone_duskwood.cpp` | 162 | `prefix` |
| `scripts/EasternKingdoms/zone_eastern_plaguelands.cpp` | 54 | `prefix` |
| `scripts/EasternKingdoms/zone_elwynn_forest.cpp` | 728 | `prefix` |
| `scripts/EasternKingdoms/zone_eversong_woods.cpp` | 212 | `prefix` |
| `scripts/EasternKingdoms/zone_hinterlands.cpp` | 142 | `prefix` |
| `scripts/EasternKingdoms/zone_ironforge.cpp` | 20 | `prefix` |
| `scripts/EasternKingdoms/zone_isle_of_queldanas.cpp` | 422 | `prefix` |
| `scripts/EasternKingdoms/zone_redridge_mountains.cpp` | 371 | `prefix` |
| `scripts/EasternKingdoms/zone_silverpine_forest.cpp` | 5317 | `prefix` |
| `scripts/EasternKingdoms/zone_stormwind_city.cpp` | 20 | `prefix` |
| `scripts/EasternKingdoms/zone_tirisfal_glades.cpp` | 20 | `prefix` |
| `scripts/EasternKingdoms/zone_undercity.cpp` | 359 | `prefix` |
| `scripts/Events/brewfest.cpp` | 650 | `prefix` |
| `scripts/Events/childrens_week.cpp` | 1123 | `prefix` |
| `scripts/Events/darkmoon_faire.cpp` | 168 | `prefix` |
| `scripts/Events/events_script_loader.cpp` | 48 | `prefix` |
| `scripts/Events/fireworks_show.cpp` | 906 | `prefix` |
| `scripts/Events/hallows_end.cpp` | 328 | `prefix` |
| `scripts/Events/love_is_in_the_air.cpp` | 338 | `prefix` |
| `scripts/Events/lunar_festival.cpp` | 480 | `prefix` |
| `scripts/Events/midsummer.cpp` | 439 | `prefix` |
| `scripts/Events/operation_gnomeregan.cpp` | 74 | `prefix` |
| `scripts/Events/pilgrims_bounty.cpp` | 470 | `prefix` |
| `scripts/Events/winter_veil.cpp` | 168 | `prefix` |
| `scripts/Events/zalazane_fall.cpp` | 406 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/blackfathom_deeps.cpp` | 236 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/blackfathom_deeps.h` | 82 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/boss_aku_mai.cpp` | 87 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/boss_gelihast.cpp` | 63 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/boss_kelris.cpp` | 112 | `prefix` |
| `scripts/Kalimdor/BlackfathomDeeps/instance_blackfathom_deeps.cpp` | 261 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/boss_anetheron.cpp` | 293 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/boss_archimonde.cpp` | 641 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/boss_azgalor.cpp` | 272 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/boss_kazrogal.cpp` | 236 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/boss_rage_winterchill.cpp` | 163 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjal.cpp` | 291 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjal.h` | 100 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjalAI.cpp` | 1103 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjalAI.h` | 211 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjal_trash.cpp` | 1523 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/hyjal_trash.h` | 48 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/BattleForMountHyjal/instance_hyjal.cpp` | 251 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/boss_chrono_lord_epoch.cpp` | 165 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/boss_infinite_corruptor.cpp` | 163 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/boss_mal_ganis.cpp` | 188 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/boss_meathook.cpp` | 120 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/boss_salramm_the_fleshcrafter.cpp` | 162 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/culling_of_stratholme.cpp` | 1484 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/culling_of_stratholme.h` | 176 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/instance_culling_of_stratholme.cpp` | 823 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/CullingOfStratholme/npc_arthas.cpp` | 1677 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/boss_captain_skarloc.cpp` | 154 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/boss_epoch_hunter.cpp` | 136 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/boss_leutenant_drake.cpp` | 186 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/instance_old_hillsbrad.cpp` | 195 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/old_hillsbrad.cpp` | 650 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/EscapeFromDurnholdeKeep/old_hillsbrad.h` | 66 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/boss_aeonus.cpp` | 139 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/boss_chrono_lord_deja.cpp` | 144 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/boss_temporus.cpp` | 142 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/instance_the_black_morass.cpp` | 337 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/the_black_morass.cpp` | 382 | `prefix` |
| `scripts/Kalimdor/CavernsOfTime/TheBlackMorass/the_black_morass.h` | 78 | `prefix` |
| `scripts/Kalimdor/DireMaul/diremaul.h` | 81 | `prefix` |
| `scripts/Kalimdor/DireMaul/instance_dire_maul.cpp` | 311 | `prefix` |
| `scripts/Kalimdor/Maraudon/boss_celebras_the_cursed.cpp` | 115 | `prefix` |
| `scripts/Kalimdor/Maraudon/boss_landslide.cpp` | 110 | `prefix` |
| `scripts/Kalimdor/Maraudon/boss_noxxion.cpp` | 147 | `prefix` |
| `scripts/Kalimdor/Maraudon/boss_princess_theradras.cpp` | 125 | `prefix` |
| `scripts/Kalimdor/Maraudon/instance_maraudon.cpp` | 78 | `prefix` |
| `scripts/Kalimdor/Maraudon/maraudon.h` | 53 | `prefix` |
| `scripts/Kalimdor/OnyxiasLair/boss_onyxia.cpp` | 491 | `prefix` |
| `scripts/Kalimdor/OnyxiasLair/instance_onyxias_lair.cpp` | 282 | `prefix` |
| `scripts/Kalimdor/OnyxiasLair/onyxias_lair.h` | 84 | `prefix` |
| `scripts/Kalimdor/RagefireChasm/instance_ragefire_chasm.cpp` | 47 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/boss_amnennar_the_coldbringer.cpp` | 159 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/boss_glutton.cpp` | 105 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/boss_mordresh_fire_eye.cpp` | 138 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/boss_tuten_kash.cpp` | 109 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/instance_razorfen_downs.cpp` | 185 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/razorfen_downs.cpp` | 402 | `prefix` |
| `scripts/Kalimdor/RazorfenDowns/razorfen_downs.h` | 72 | `prefix` |
| `scripts/Kalimdor/RazorfenKraul/instance_razorfen_kraul.cpp` | 113 | `prefix` |
| `scripts/Kalimdor/RazorfenKraul/razorfen_kraul.cpp` | 267 | `prefix` |
| `scripts/Kalimdor/RazorfenKraul/razorfen_kraul.h` | 54 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_ayamiss.cpp` | 302 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_buru.cpp` | 284 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_kurinnaxx.cpp` | 145 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_moam.cpp` | 191 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_ossirian.cpp` | 323 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/boss_rajaxx.cpp` | 142 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/instance_ruins_of_ahnqiraj.cpp` | 128 | `prefix` |
| `scripts/Kalimdor/RuinsOfAhnQiraj/ruins_of_ahnqiraj.h` | 69 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_bug_trio.cpp` | 332 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_cthun.cpp` | 1304 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_fankriss.cpp` | 214 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_huhuran.cpp` | 159 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_ouro.cpp` | 151 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_sartura.cpp` | 335 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_skeram.cpp` | 278 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_twinemperors.cpp` | 601 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/boss_viscidus.cpp` | 317 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/instance_temple_of_ahnqiraj.cpp` | 164 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/mob_anubisath_sentinel.cpp` | 265 | `prefix` |
| `scripts/Kalimdor/TempleOfAhnQiraj/temple_of_ahnqiraj.h` | 92 | `prefix` |
| `scripts/Kalimdor/WailingCaverns/instance_wailing_caverns.cpp` | 134 | `prefix` |
| `scripts/Kalimdor/WailingCaverns/wailing_caverns.cpp` | 380 | `prefix` |
| `scripts/Kalimdor/WailingCaverns/wailing_caverns.h` | 69 | `prefix` |
| `scripts/Kalimdor/ZulFarrak/boss_zum_rah.cpp` | 157 | `prefix` |
| `scripts/Kalimdor/ZulFarrak/instance_zulfarrak.cpp` | 412 | `prefix` |
| `scripts/Kalimdor/ZulFarrak/zulfarrak.cpp` | 476 | `prefix` |
| `scripts/Kalimdor/ZulFarrak/zulfarrak.h` | 81 | `prefix` |
| `scripts/Kalimdor/kalimdor_script_loader.cpp` | 240 | `prefix` |
| `scripts/Kalimdor/zone_ashenvale.cpp` | 411 | `prefix` |
| `scripts/Kalimdor/zone_azshara.cpp` | 20 | `prefix` |
| `scripts/Kalimdor/zone_azuremyst_isle.cpp` | 671 | `prefix` |
| `scripts/Kalimdor/zone_bloodmyst_isle.cpp` | 823 | `prefix` |
| `scripts/Kalimdor/zone_darkshore.cpp` | 20 | `prefix` |
| `scripts/Kalimdor/zone_desolace.cpp` | 136 | `prefix` |
| `scripts/Kalimdor/zone_durotar.cpp` | 184 | `prefix` |
| `scripts/Kalimdor/zone_dustwallow_marsh.cpp` | 155 | `prefix` |
| `scripts/Kalimdor/zone_felwood.cpp` | 258 | `prefix` |
| `scripts/Kalimdor/zone_feralas.cpp` | 21 | `prefix` |
| `scripts/Kalimdor/zone_moonglade.cpp` | 27 | `prefix` |
| `scripts/Kalimdor/zone_mulgore.cpp` | 80 | `prefix` |
| `scripts/Kalimdor/zone_orgrimmar.cpp` | 20 | `prefix` |
| `scripts/Kalimdor/zone_silithus.cpp` | 1479 | `prefix` |
| `scripts/Kalimdor/zone_tanaris.cpp` | 194 | `prefix` |
| `scripts/Kalimdor/zone_the_barrens.cpp` | 518 | `prefix` |
| `scripts/Kalimdor/zone_thunder_bluff.cpp` | 165 | `prefix` |
| `scripts/Kalimdor/zone_winterspring.cpp` | 605 | `prefix` |
| `scripts/Maelstrom/Stonecore/boss_corborus.cpp` | 320 | `prefix` |
| `scripts/Maelstrom/Stonecore/boss_high_priestess_azil.cpp` | 713 | `prefix` |
| `scripts/Maelstrom/Stonecore/boss_ozruk.cpp` | 269 | `prefix` |
| `scripts/Maelstrom/Stonecore/boss_slabhide.cpp` | 581 | `prefix` |
| `scripts/Maelstrom/Stonecore/instance_stonecore.cpp` | 245 | `prefix` |
| `scripts/Maelstrom/Stonecore/stonecore.cpp` | 403 | `prefix` |
| `scripts/Maelstrom/Stonecore/stonecore.h` | 85 | `prefix` |
| `scripts/Maelstrom/kezan.cpp` | 18 | `prefix` |
| `scripts/Maelstrom/maelstrom_script_loader.cpp` | 40 | `prefix` |
| `scripts/Maelstrom/zone_deepholm.cpp` | 59 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/ahnkahet.cpp` | 117 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/ahnkahet.h` | 90 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/boss_amanitar.cpp` | 288 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/boss_elder_nadox.cpp` | 273 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/boss_herald_volazj.cpp` | 784 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/boss_jedoga_shadowseeker.cpp` | 506 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/boss_prince_taldaram.cpp` | 456 | `prefix` |
| `scripts/Northrend/AzjolNerub/Ahnkahet/instance_ahnkahet.cpp` | 162 | `prefix` |
| `scripts/Northrend/AzjolNerub/AzjolNerub/azjol_nerub.h` | 77 | `prefix` |
| `scripts/Northrend/AzjolNerub/AzjolNerub/boss_anubarak.cpp` | 647 | `prefix` |
| `scripts/Northrend/AzjolNerub/AzjolNerub/boss_hadronox.cpp` | 1040 | `prefix` |
| `scripts/Northrend/AzjolNerub/AzjolNerub/boss_krikthir_the_gatewatcher.cpp` | 923 | `prefix` |
| `scripts/Northrend/AzjolNerub/AzjolNerub/instance_azjol_nerub.cpp` | 146 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/ObsidianSanctum/boss_sartharion.cpp` | 508 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/ObsidianSanctum/instance_obsidian_sanctum.cpp` | 133 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/ObsidianSanctum/obsidian_sanctum.cpp` | 936 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/ObsidianSanctum/obsidian_sanctum.h` | 59 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/boss_baltharus_the_warborn.cpp` | 329 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/boss_general_zarithrian.cpp` | 254 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/boss_halion.cpp` | 1925 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/boss_saviana_ragefire.cpp` | 259 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/instance_ruby_sanctum.cpp` | 222 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/ruby_sanctum.cpp` | 203 | `prefix` |
| `scripts/Northrend/ChamberOfAspects/RubySanctum/ruby_sanctum.h` | 140 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/boss_argent_challenge.cpp` | 678 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/boss_black_knight.cpp` | 428 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/boss_grand_champions.cpp` | 857 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/instance_trial_of_the_champion.cpp` | 290 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/trial_of_the_champion.cpp` | 507 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheChampion/trial_of_the_champion.h` | 139 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/boss_anubarak_trial.cpp` | 887 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/boss_faction_champions.cpp` | 2184 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/boss_lord_jaraxxus.cpp` | 552 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/boss_northrend_beasts.cpp` | 1358 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/boss_twin_valkyr.cpp` | 854 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/instance_trial_of_the_crusader.cpp` | 550 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/trial_of_the_crusader.cpp` | 908 | `prefix` |
| `scripts/Northrend/CrusadersColiseum/TrialOfTheCrusader/trial_of_the_crusader.h` | 291 | `prefix` |
| `scripts/Northrend/DraktharonKeep/boss_king_dred.cpp` | 273 | `prefix` |
| `scripts/Northrend/DraktharonKeep/boss_novos.cpp` | 394 | `prefix` |
| `scripts/Northrend/DraktharonKeep/boss_tharon_ja.cpp` | 220 | `prefix` |
| `scripts/Northrend/DraktharonKeep/boss_trollgore.cpp` | 301 | `prefix` |
| `scripts/Northrend/DraktharonKeep/drak_tharon_keep.cpp` | 51 | `prefix` |
| `scripts/Northrend/DraktharonKeep/drak_tharon_keep.h` | 97 | `prefix` |
| `scripts/Northrend/DraktharonKeep/instance_drak_tharon_keep.cpp` | 185 | `prefix` |
| `scripts/Northrend/FrozenHalls/ForgeOfSouls/boss_bronjahm.cpp` | 339 | `prefix` |
| `scripts/Northrend/FrozenHalls/ForgeOfSouls/boss_devourer_of_souls.cpp` | 448 | `prefix` |
| `scripts/Northrend/FrozenHalls/ForgeOfSouls/forge_of_souls.cpp` | 288 | `prefix` |
| `scripts/Northrend/FrozenHalls/ForgeOfSouls/forge_of_souls.h` | 68 | `prefix` |
| `scripts/Northrend/FrozenHalls/ForgeOfSouls/instance_forge_of_souls.cpp` | 142 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/boss_falric.cpp` | 158 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/boss_horAI.cpp` | 53 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/boss_horAI.h` | 32 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/boss_marwyn.cpp` | 174 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/halls_of_reflection.cpp` | 2881 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/halls_of_reflection.h` | 206 | `prefix` |
| `scripts/Northrend/FrozenHalls/HallsOfReflection/instance_halls_of_reflection.cpp` | 796 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/boss_forgemaster_garfrost.cpp` | 331 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/boss_krickandick.cpp` | 662 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/boss_scourgelord_tyrannus.cpp` | 512 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/instance_pit_of_saron.cpp` | 311 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/pit_of_saron.cpp` | 302 | `prefix` |
| `scripts/Northrend/FrozenHalls/PitOfSaron/pit_of_saron.h` | 134 | `prefix` |
| `scripts/Northrend/Gundrak/boss_drakkari_colossus.cpp` | 450 | `prefix` |
| `scripts/Northrend/Gundrak/boss_eck.cpp` | 114 | `prefix` |
| `scripts/Northrend/Gundrak/boss_gal_darah.cpp` | 327 | `prefix` |
| `scripts/Northrend/Gundrak/boss_moorabi.cpp` | 233 | `prefix` |
| `scripts/Northrend/Gundrak/boss_slad_ran.cpp` | 303 | `prefix` |
| `scripts/Northrend/Gundrak/gundrak.h` | 102 | `prefix` |
| `scripts/Northrend/Gundrak/instance_gundrak.cpp` | 367 | `prefix` |
| `scripts/Northrend/IsleOfConquest/boss_ioc_horde_alliance.cpp` | 123 | `prefix` |
| `scripts/Northrend/IsleOfConquest/isle_of_conquest.cpp` | 257 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_anubrekhan.cpp` | 257 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_faerlina.cpp` | 266 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_four_horsemen.cpp` | 707 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_gluth.cpp` | 437 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_gothik.cpp` | 906 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_grobbulus.cpp` | 249 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_heigan.cpp` | 255 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_kelthuzad.cpp` | 977 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_loatheb.cpp` | 179 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_maexxna.cpp` | 232 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_noth.cpp` | 329 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_patchwerk.cpp` | 183 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_razuvious.cpp` | 214 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_sapphiron.cpp` | 643 | `prefix` |
| `scripts/Northrend/Naxxramas/boss_thaddius.cpp` | 1182 | `prefix` |
| `scripts/Northrend/Naxxramas/instance_naxxramas.cpp` | 621 | `prefix` |
| `scripts/Northrend/Naxxramas/naxxramas.cpp` | 135 | `prefix` |
| `scripts/Northrend/Naxxramas/naxxramas.h` | 224 | `prefix` |
| `scripts/Northrend/Nexus/EyeOfEternity/boss_malygos.cpp` | 2151 | `prefix` |
| `scripts/Northrend/Nexus/EyeOfEternity/eye_of_eternity.h` | 98 | `prefix` |
| `scripts/Northrend/Nexus/EyeOfEternity/instance_eye_of_eternity.cpp` | 294 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/boss_anomalus.cpp` | 273 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/boss_keristrasza.cpp` | 277 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/boss_magus_telestra.cpp` | 400 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/boss_nexus_commanders.cpp` | 99 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/boss_ormorok.cpp` | 314 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/instance_nexus.cpp` | 187 | `prefix` |
| `scripts/Northrend/Nexus/Nexus/nexus.h` | 77 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/boss_drakos.cpp` | 168 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/boss_eregos.cpp` | 288 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/boss_urom.cpp` | 362 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/boss_varos.cpp` | 333 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/instance_oculus.cpp` | 337 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/oculus.cpp` | 558 | `prefix` |
| `scripts/Northrend/Nexus/Oculus/oculus.h` | 118 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/boss_ingvar_the_plunderer.cpp` | 427 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/boss_keleseth.cpp` | 341 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/boss_skarvald_dalronn.cpp` | 280 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/instance_utgarde_keep.cpp` | 208 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/utgarde_keep.cpp` | 263 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardeKeep/utgarde_keep.h` | 98 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/boss_palehoof.cpp` | 607 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/boss_skadi.cpp` | 848 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/boss_svala.cpp` | 567 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/boss_ymiron.cpp` | 343 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/instance_utgarde_pinnacle.cpp` | 122 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/utgarde_pinnacle.cpp` | 72 | `prefix` |
| `scripts/Northrend/UtgardeKeep/UtgardePinnacle/utgarde_pinnacle.h` | 109 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/boss_archavon.cpp` | 153 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/boss_emalon.cpp` | 252 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/boss_koralon.cpp` | 178 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/boss_toravon.cpp` | 166 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/instance_vault_of_archavon.cpp` | 132 | `prefix` |
| `scripts/Northrend/VaultOfArchavon/vault_of_archavon.h` | 63 | `prefix` |
| `scripts/Northrend/VioletHold/boss_cyanigosa.cpp` | 159 | `prefix` |
| `scripts/Northrend/VioletHold/boss_erekem.cpp` | 298 | `prefix` |
| `scripts/Northrend/VioletHold/boss_ichoron.cpp` | 422 | `prefix` |
| `scripts/Northrend/VioletHold/boss_lavanthor.cpp` | 95 | `prefix` |
| `scripts/Northrend/VioletHold/boss_moragg.cpp` | 169 | `prefix` |
| `scripts/Northrend/VioletHold/boss_xevozz.cpp` | 257 | `prefix` |
| `scripts/Northrend/VioletHold/boss_zuramat.cpp` | 209 | `prefix` |
| `scripts/Northrend/VioletHold/instance_violet_hold.cpp` | 956 | `prefix` |
| `scripts/Northrend/VioletHold/violet_hold.cpp` | 1445 | `prefix` |
| `scripts/Northrend/VioletHold/violet_hold.h` | 167 | `prefix` |
| `scripts/Northrend/northrend_script_loader.cpp` | 409 | `prefix` |
| `scripts/Northrend/zone_borean_tundra.cpp` | 1722 | `prefix` |
| `scripts/Northrend/zone_dalaran.cpp` | 260 | `prefix` |
| `scripts/Northrend/zone_dragonblight.cpp` | 976 | `prefix` |
| `scripts/Northrend/zone_grizzly_hills.cpp` | 952 | `prefix` |
| `scripts/Northrend/zone_howling_fjord.cpp` | 552 | `prefix` |
| `scripts/Northrend/zone_icecrown.cpp` | 936 | `prefix` |
| `scripts/Northrend/zone_sholazar_basin.cpp` | 797 | `prefix` |
| `scripts/Northrend/zone_storm_peaks.cpp` | 1354 | `prefix` |
| `scripts/Northrend/zone_wintergrasp.cpp` | 551 | `prefix` |
| `scripts/Northrend/zone_zuldrak.cpp` | 1034 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPHP.cpp` | 319 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPHP.h` | 178 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPNA.cpp` | 518 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPNA.h` | 198 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPSI.cpp` | 205 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPSI.h` | 56 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPScriptLoader.cpp` | 34 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPTF.cpp` | 345 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPTF.h` | 150 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPZM.cpp` | 481 | `prefix` |
| `scripts/OutdoorPvP/OutdoorPvPZM.h` | 204 | `prefix` |
| `scripts/Outland/Auchindoun/AuchenaiCrypts/auchenai_crypts.h` | 43 | `prefix` |
| `scripts/Outland/Auchindoun/AuchenaiCrypts/boss_exarch_maladaar.cpp` | 371 | `prefix` |
| `scripts/Outland/Auchindoun/AuchenaiCrypts/boss_shirrak_the_dead_watcher.cpp` | 200 | `prefix` |
| `scripts/Outland/Auchindoun/AuchenaiCrypts/instance_auchenai_crypts.cpp` | 52 | `prefix` |
| `scripts/Outland/Auchindoun/ManaTombs/boss_nexusprince_shaffar.cpp` | 347 | `prefix` |
| `scripts/Outland/Auchindoun/ManaTombs/boss_pandemonius.cpp` | 116 | `prefix` |
| `scripts/Outland/Auchindoun/ManaTombs/instance_mana_tombs.cpp` | 61 | `prefix` |
| `scripts/Outland/Auchindoun/ManaTombs/mana_tombs.h` | 50 | `prefix` |
| `scripts/Outland/Auchindoun/SethekkHalls/boss_anzu.cpp` | 164 | `prefix` |
| `scripts/Outland/Auchindoun/SethekkHalls/boss_darkweaver_syth.cpp` | 204 | `prefix` |
| `scripts/Outland/Auchindoun/SethekkHalls/boss_talon_king_ikiss.cpp` | 189 | `prefix` |
| `scripts/Outland/Auchindoun/SethekkHalls/instance_sethekk_halls.cpp` | 103 | `prefix` |
| `scripts/Outland/Auchindoun/SethekkHalls/sethekk_halls.h` | 59 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/boss_ambassador_hellmaw.cpp` | 178 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/boss_blackheart_the_inciter.cpp` | 252 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/boss_grandmaster_vorpil.cpp` | 245 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/boss_murmur.cpp` | 288 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/instance_shadow_labyrinth.cpp` | 203 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/shadow_labyrinth.cpp` | 54 | `prefix` |
| `scripts/Outland/Auchindoun/ShadowLabyrinth/shadow_labyrinth.h` | 76 | `prefix` |
| `scripts/Outland/BlackTemple/black_temple.cpp` | 275 | `prefix` |
| `scripts/Outland/BlackTemple/black_temple.h` | 151 | `prefix` |
| `scripts/Outland/BlackTemple/boss_gurtogg_bloodboil.cpp` | 360 | `prefix` |
| `scripts/Outland/BlackTemple/boss_illidan.cpp` | 2341 | `prefix` |
| `scripts/Outland/BlackTemple/boss_illidari_council.cpp` | 806 | `prefix` |
| `scripts/Outland/BlackTemple/boss_mother_shahraz.cpp` | 319 | `prefix` |
| `scripts/Outland/BlackTemple/boss_reliquary_of_souls.cpp` | 823 | `prefix` |
| `scripts/Outland/BlackTemple/boss_shade_of_akama.cpp` | 1118 | `prefix` |
| `scripts/Outland/BlackTemple/boss_supremus.cpp` | 226 | `prefix` |
| `scripts/Outland/BlackTemple/boss_teron_gorefiend.cpp` | 411 | `prefix` |
| `scripts/Outland/BlackTemple/boss_warlord_najentus.cpp` | 233 | `prefix` |
| `scripts/Outland/BlackTemple/instance_black_temple.cpp` | 251 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_fathomlord_karathress.cpp` | 677 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_hydross_the_unstable.cpp` | 381 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_lady_vashj.cpp` | 889 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_leotheras_the_blind.cpp` | 769 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_lurker_below.cpp` | 439 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/boss_morogrim_tidewalker.cpp` | 336 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/instance_serpent_shrine.cpp` | 395 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SerpentShrine/serpent_shrine.h` | 75 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SteamVault/boss_hydromancer_thespia.cpp` | 184 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SteamVault/boss_mekgineer_steamrigger.cpp` | 274 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SteamVault/boss_warlord_kalithresh.cpp` | 206 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SteamVault/instance_steam_vault.cpp` | 159 | `prefix` |
| `scripts/Outland/CoilfangReservoir/SteamVault/steam_vault.h` | 72 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/boss_ahune.cpp` | 890 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/boss_mennu_the_betrayer.cpp` | 131 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/boss_quagmirran.cpp` | 114 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/boss_rokmar_the_crackler.cpp` | 125 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/instance_the_slave_pens.cpp` | 114 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheSlavePens/the_slave_pens.h` | 78 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheUnderbog/boss_hungarfen.cpp` | 163 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheUnderbog/boss_the_black_stalker.cpp` | 262 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheUnderbog/instance_the_underbog.cpp` | 68 | `prefix` |
| `scripts/Outland/CoilfangReservoir/TheUnderbog/the_underbog.h` | 50 | `prefix` |
| `scripts/Outland/GruulsLair/boss_gruul.cpp` | 346 | `prefix` |
| `scripts/Outland/GruulsLair/boss_high_king_maulgar.cpp` | 579 | `prefix` |
| `scripts/Outland/GruulsLair/gruuls_lair.h` | 56 | `prefix` |
| `scripts/Outland/GruulsLair/instance_gruuls_lair.cpp` | 107 | `prefix` |
| `scripts/Outland/HellfireCitadel/BloodFurnace/blood_furnace.h` | 95 | `prefix` |
| `scripts/Outland/HellfireCitadel/BloodFurnace/boss_broggok.cpp` | 318 | `prefix` |
| `scripts/Outland/HellfireCitadel/BloodFurnace/boss_kelidan_the_breaker.cpp` | 370 | `prefix` |
| `scripts/Outland/HellfireCitadel/BloodFurnace/boss_the_maker.cpp` | 114 | `prefix` |
| `scripts/Outland/HellfireCitadel/BloodFurnace/instance_blood_furnace.cpp` | 367 | `prefix` |
| `scripts/Outland/HellfireCitadel/HellfireRamparts/boss_omor_the_unscarred.cpp` | 237 | `prefix` |
| `scripts/Outland/HellfireCitadel/HellfireRamparts/boss_vazruden_the_herald.cpp` | 531 | `prefix` |
| `scripts/Outland/HellfireCitadel/HellfireRamparts/boss_watchkeeper_gargolmar.cpp` | 182 | `prefix` |
| `scripts/Outland/HellfireCitadel/HellfireRamparts/hellfire_ramparts.h` | 57 | `prefix` |
| `scripts/Outland/HellfireCitadel/HellfireRamparts/instance_hellfire_ramparts.cpp` | 95 | `prefix` |
| `scripts/Outland/HellfireCitadel/MagtheridonsLair/boss_magtheridon.cpp` | 529 | `prefix` |
| `scripts/Outland/HellfireCitadel/MagtheridonsLair/instance_magtheridons_lair.cpp` | 153 | `prefix` |
| `scripts/Outland/HellfireCitadel/MagtheridonsLair/magtheridons_lair.h` | 86 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/boss_nethekurse.cpp` | 401 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/boss_warbringer_omrogg.cpp` | 453 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/boss_warchief_kargath_bladefist.cpp` | 341 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/instance_shattered_halls.cpp` | 259 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/shattered_halls.cpp` | 257 | `prefix` |
| `scripts/Outland/HellfireCitadel/ShatteredHalls/shattered_halls.h` | 124 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/boss_alar.cpp` | 574 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/boss_astromancer.cpp` | 504 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/boss_kaelthas.cpp` | 1423 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/boss_void_reaver.cpp` | 151 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/instance_the_eye.cpp` | 92 | `prefix` |
| `scripts/Outland/TempestKeep/Eye/the_eye.h` | 76 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/boss_gatewatcher_gyrokill.cpp` | 117 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/boss_gatewatcher_ironhand.cpp` | 123 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/boss_mechano_lord_capacitus.cpp` | 241 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/boss_nethermancer_sepethrea.cpp` | 226 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/boss_pathaleon_the_calculator.cpp` | 228 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/instance_mechanar.cpp` | 84 | `prefix` |
| `scripts/Outland/TempestKeep/Mechanar/mechanar.h` | 52 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/arcatraz.cpp` | 501 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/arcatraz.h` | 81 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/boss_dalliah_the_doomsayer.cpp` | 191 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/boss_harbinger_skyriss.cpp` | 287 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/boss_wrath_scryer_soccothrates.cpp` | 285 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/boss_zereketh_the_unbound.cpp` | 115 | `prefix` |
| `scripts/Outland/TempestKeep/arcatraz/instance_arcatraz.cpp` | 221 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/boss_commander_sarannis.cpp` | 197 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/boss_high_botanist_freywinn.cpp` | 150 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/boss_laj.cpp` | 152 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/boss_thorngrin_the_tender.cpp` | 166 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/boss_warp_splinter.cpp` | 168 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/instance_the_botanica.cpp` | 128 | `prefix` |
| `scripts/Outland/TempestKeep/botanica/the_botanica.h` | 54 | `prefix` |
| `scripts/Outland/boss_doomlord_kazzak.cpp` | 226 | `prefix` |
| `scripts/Outland/boss_doomwalker.cpp` | 164 | `prefix` |
| `scripts/Outland/outland_script_loader.cpp` | 260 | `prefix` |
| `scripts/Outland/zone_blades_edge_mountains.cpp` | 1019 | `prefix` |
| `scripts/Outland/zone_hellfire_peninsula.cpp` | 855 | `prefix` |
| `scripts/Outland/zone_nagrand.cpp` | 793 | `prefix` |
| `scripts/Outland/zone_netherstorm.cpp` | 536 | `prefix` |
| `scripts/Outland/zone_shadowmoon_valley.cpp` | 1659 | `prefix` |
| `scripts/Outland/zone_terokkar_forest.cpp` | 312 | `prefix` |
| `scripts/Pet/pet_dk.cpp` | 122 | `prefix` |
| `scripts/Pet/pet_generic.cpp` | 292 | `prefix` |
| `scripts/Pet/pet_hunter.cpp` | 128 | `prefix` |
| `scripts/Pet/pet_mage.cpp` | 198 | `prefix` |
| `scripts/Pet/pet_priest.cpp` | 99 | `prefix` |
| `scripts/Pet/pet_script_loader.cpp` | 36 | `prefix` |
| `scripts/Pet/pet_shaman.cpp` | 124 | `prefix` |
| `scripts/ScriptLoader.h` | 23 | `prefix` |
| `scripts/ScriptPCH.h` | 38 | `prefix` |
| `scripts/Spells/spell_dk.cpp` | 924 | `prefix` |
| `scripts/Spells/spell_druid.cpp` | 2137 | `prefix` |
| `scripts/Spells/spell_generic.cpp` | 5538 | `prefix` |
| `scripts/Spells/spell_hunter.cpp` | 809 | `prefix` |
| `scripts/Spells/spell_item.cpp` | 4799 | `prefix` |
| `scripts/Spells/spell_mage.cpp` | 1551 | `prefix` |
| `scripts/Spells/spell_paladin.cpp` | 927 | `prefix` |
| `scripts/Spells/spell_pet.cpp` | 1631 | `prefix` |
| `scripts/Spells/spell_priest.cpp` | 2809 | `prefix` |
| `scripts/Spells/spell_quest.cpp` | 1959 | `prefix` |
| `scripts/Spells/spell_rogue.cpp` | 1069 | `prefix` |
| `scripts/Spells/spell_script_loader.cpp` | 50 | `prefix` |
| `scripts/Spells/spell_shaman.cpp` | 1051 | `prefix` |
| `scripts/Spells/spell_warlock.cpp` | 1041 | `prefix` |
| `scripts/Spells/spell_warrior.cpp` | 850 | `prefix` |
| `scripts/World/achievement_scripts.cpp` | 147 | `prefix` |
| `scripts/World/action_ip_logger.cpp` | 319 | `prefix` |
| `scripts/World/areatrigger_scripts.cpp` | 476 | `prefix` |
| `scripts/World/boosted_xp.cpp` | 51 | `prefix` |
| `scripts/World/boss_emerald_dragons.cpp` | 820 | `prefix` |
| `scripts/World/chat_log.cpp` | 149 | `prefix` |
| `scripts/World/conversation_scripts.cpp` | 53 | `prefix` |
| `scripts/World/duel_reset.cpp` | 134 | `prefix` |
| `scripts/World/go_scripts.cpp` | 1169 | `prefix` |
| `scripts/World/item_scripts.cpp` | 239 | `prefix` |
| `scripts/World/npc_guard.cpp` | 239 | `prefix` |
| `scripts/World/npc_professions.cpp` | 1311 | `prefix` |
| `scripts/World/npcs_special.cpp` | 2315 | `prefix` |
| `scripts/World/scene_scripts.cpp` | 42 | `prefix` |
| `scripts/World/world_script_loader.cpp` | 61 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

All paths relative to `/home/server/woltk-trinity-legacy/src/server/scripts/`. Sizes are in `.cpp` files (.h are tiny instance-data accessors).

| Subdirectory | `.cpp` count | Purpose |
|---|---|---|
| `Battlefield/` | 2 (+1 `.h`, +1 loader) | `BattlefieldWG` — Wintergrasp implementation. The only content-side Battlefield in WoLK. |
| `Commands/` | 42 + loader | Every `.gm`, `.cheat`, `.tele`, `.npc`, `.account`, `.ban`, `.gobject`, etc. GM chat command (one file per `cs_*.cpp` topic). |
| `Custom/` | 0 + loader | Empty — placeholder for site-local custom scripts. |
| `EasternKingdoms/` | 173 | Old-world (1.x) and BC zones in EK + their dungeons/raids: Karazhan, Sunwell Plateau, ZulAman, ZulGurub, Magisters' Terrace, Scarlet Monastery/Enclave, Stratholme, Scholomance, Shadowfang Keep, Deadmines, Gnomeregan, The Stockade, Sunken Temple, Uldaman, Blackrock Mountain (BRD/BWL/MC/UBRS/LBRS), 16 zone files. |
| `Events/` | 13 (+1 loader) | The 12 holiday/world events scripted on the content side: brewfest, childrens_week, darkmoon_faire, fireworks_show, hallows_end, love_is_in_the_air, lunar_festival, midsummer, operation_gnomeregan, pilgrims_bounty, winter_veil, zalazane_fall. (See `events.md` for the GameEventMgr scheduler that drives them.) |
| `Kalimdor/` | 173 | Old-world Kalimdor + BC: Onyxia's Lair, Temple/Ruins of Ahn'Qiraj, Dire Maul, Maraudon, Wailing Caverns, Razorfen Downs/Kraul, Blackfathom Deeps, Ragefire Chasm, ZulFarrak, Caverns of Time (Old Hillsbrad, Black Morass, CoS, Hyjal, Culling, Drak'Tharon, Battle for Mt. Hyjal), 18 zone files. |
| `Maelstrom/` | 2 + 1 subdir | Cataclysm-leakage (Stonecore, Deepholm, Kezan). Almost entirely vestigial in WoLK. |
| `Northrend/` | 169 | The WoLK content. **The single most important directory** for this expansion: Icecrown Citadel, Ulduar, Trial of the Crusader / Champion, Naxxramas, all 5-mans (Azjol-Nerub, Ahnkahet, Drak'Tharon Keep, Frozen Halls, Gundrak, Nexus, Eye of Eternity, Oculus, Utgarde Keep/Pinnacle, Violet Hold), Vault of Archavon, Chamber of Aspects, Onyxia (re-tuned), Isle of Conquest BG, plus 10 Northrend zone files (Borean Tundra, Dalaran, Dragonblight, Grizzly Hills, Howling Fjord, Icecrown, Sholazar Basin, Storm Peaks, Wintergrasp, Zul'Drak). |
| `OutdoorPvP/` | 5 (HP, NA, SI, TF, ZM) + loader | The five Outland outdoor PvP zones (Hellfire Peninsula, Nagrand, Silithus, Terokkar Forest, Zangarmarsh). |
| `Outland/` | 91 | BC content: Black Temple, Coilfang Reservoir (SP/SH/SV/UB), Tempest Keep (Eye/Mech/Bot/Arc), Hellfire Citadel (HFR/SH/BF/MT), Gruul's Lair, Auchindoun (AC/SH/MT/SE), 6 zone files, two Outland world bosses (Doomlord Kazzak, Doomwalker). |
| `Pet/` | 6 (+ loader) | Class-specific pet AI overrides: DK ghoul/army-of-the-dead, hunter pets, mage water elemental, priest shadowfiend, shaman elementals, generic guardians. |
| `Spells/` | 15 (+ loader) | The big one: per-class scripted spells (`spell_dk.cpp`, `spell_druid.cpp`, …, `spell_warrior.cpp`), plus `spell_generic.cpp`, `spell_item.cpp`, `spell_quest.cpp`, `spell_pet.cpp`. Each file packs hundreds of `class spell_xxx : public SpellScript` / `AuraScript` definitions implementing per-spell scripted behavior (proc handlers, custom target selection, conditional damage, etc.). |
| `World/` | 15 | Cross-zone scripts: `npcs_special.cpp` (huge — innkeepers, banker, guards, helpers), `npc_professions.cpp`, `npc_guard.cpp`, `boss_emerald_dragons.cpp`, `go_scripts.cpp` (generic gameobject use), `item_scripts.cpp`, `areatrigger_scripts.cpp`, `achievement_scripts.cpp`, `chat_log.cpp`, `action_ip_logger.cpp`, `boosted_xp.cpp`, `duel_reset.cpp`, `conversation_scripts.cpp`, `scene_scripts.cpp`. |

### Headline file sizes (Northrend / WoLK content)

| File | Lines | Notes |
|---|---|---|
| `Northrend/IcecrownCitadel/boss_the_lich_king.cpp` | 2815 | The 25H final boss. Multi-phase, Frostmourne Room sub-encounter, Tirion intervention. |
| `Northrend/IcecrownCitadel/boss_icecrown_gunship_battle.cpp` | 2243 | Gunship battle — vehicles, jet packs, 4 NPC factions. |
| `Northrend/IcecrownCitadel/icecrown_citadel.cpp` | 1596 | Trash + helper NPCs (gauntlets, plagueworks helpers, blood-prince council servants, wing teleports). |
| `Northrend/IcecrownCitadel/sindragosa.cpp` | 1526 | 3-phase boss with Frost Beacons / Ice Tomb / unchained magic. |
| `Northrend/IcecrownCitadel/boss_sister_svalna.cpp` | 1487 | Crimson Hall (intermission encounter on the path to Blood-Queen). |
| `Northrend/IcecrownCitadel/boss_professor_putricide.cpp` | 1488 | Plague wing final boss — abomination/ooze phases, table interactions. |
| `Northrend/IcecrownCitadel/instance_icecrown_citadel.cpp` | 1420 | The full ICC `InstanceScript` state machine. |
| `Northrend/IcecrownCitadel/boss_blood_prince_council.cpp` | 1341 | Trio fight (Keleseth/Taldaram/Valanar). |
| `Northrend/IcecrownCitadel/boss_valithria_dreamwalker.cpp` | 1284 | Healing-target encounter. |
| `Northrend/IcecrownCitadel/boss_deathbringer_saurfang.cpp` | 1257 | Marks of the Fallen Champion mechanic. |
| `Northrend/IcecrownCitadel/boss_lady_deathwhisper.cpp` | 1081 | Adds + mind control phase. |
| `Northrend/IcecrownCitadel/boss_rotface.cpp` | 796 | Slime spray, ooze flood. |
| `Northrend/IcecrownCitadel/boss_blood_queen_lana_thel.cpp` | 778 | Vampiric bite chain. |
| `Northrend/IcecrownCitadel/boss_lord_marrowgar.cpp` | 697 | First boss — bone spike graveyard. |
| `Northrend/IcecrownCitadel/boss_festergut.cpp` | 457 | Plague wing first boss — inhale stacks. |
| `Northrend/IcecrownCitadel/go_icecrown_citadel_teleport.cpp` | 121 | Teleport pads. |
| **ICC subtotal** | **20,387** | Just one raid. |

Naxxramas and Ulduar have similarly heavy footprints (Naxx: 16 boss files spread across 4 wings; Ulduar: 12 main bosses plus the Halls of Stone/Lightning sub-instances under the same dir).

### Linker glue

| File | Purpose |
|---|---|
| `ScriptLoader.cpp.in.cmake` | CMake-templated source: `@TRINITY_SCRIPTS_FORWARD_DECL@` expands to ~3000 `void AddSC_xxx();` forward decls; `@TRINITY_SCRIPTS_INVOKE@` expands to the matching invocation list inside `AddScripts()`. |
| `ScriptLoader.h` | Declares `AddScripts()` and the per-loader `void AddXxxScripts()` aggregators (one per top-level subdir). |
| `<expansion>_script_loader.cpp` (one per dir, e.g. `northrend_script_loader.cpp`) | Hand-written aggregator that lists every `AddSC_*` for that directory and calls them all. |

---

## 3. Classes / Structs / Enums

There is no single canonical class hierarchy here — every file defines its own bosses. The recurring patterns are:

| Pattern | Kind | Purpose |
|---|---|---|
| `class boss_<name> : public CreatureScript { CreatureAI* GetAI(Creature*) override; class boss_<name>AI : public BossAI { … }; }` | Boss script | Standard one-encounter file. `BossAI` (in `wow-ai`) supplies `Reset/JustEngagedWith/JustDied/EnterEvadeMode/SummonedCreatureDies` plus the `events`/`summons` helpers. |
| `class instance_<dungeon> : public InstanceMapScript { InstanceScript* GetInstanceScript(InstanceMap*) override; struct instance_<dungeon>_InstanceMapScript : public InstanceScript { … }; }` | Instance script | The state machine for a 5-man/raid: tracks boss states, GUIDs of doors/teleporters/event NPCs, encounter progress, achievement criteria, save/load. |
| `class spell_<name> : public SpellScriptLoader { SpellScript* GetSpellScript() override; class spell_<name>_SpellScript : public SpellScript { void Register() override { … } }; }` | Spell script | Per-spell hook bag. Use `BeforeCast`/`OnCast`/`AfterCast`/`OnEffectHitTarget`/`OnHit`/`OnCheckCast`. |
| `class spell_<name>_aura : public AuraScript { … }` | Aura script | Periodic/proc/dispel hooks: `OnEffectApply`, `OnEffectPeriodic`, `OnEffectRemove`, `OnProc`, `AfterDispel`. |
| `class npc_<name> : public CreatureScript` | Quest/escort NPC | Simple gossip, `OnQuestAccept`, escort, helper. |
| `class go_<name> : public GameObjectScript` | GameObject script | `OnGossipHello`, `OnGossipSelect`, `OnUse`, `OnLootStateChanged`. |
| `class at_<name> : public AreaTriggerScript` | Area trigger | `OnTrigger(player, areaTriggerEntry, entered)`. |
| `class achievement_<name> : public AchievementCriteriaScript` | Achievement criterion | `OnCheck` for `MODIFIER_TREE_TYPE_REQUIRED_SCRIPT`. |
| `class <bg>_<name> : public BattlegroundMapScript` / `BattlegroundScript` | BG glue | Less common — most BG state lives in `Battleground` subclasses under `src/server/game/Battlegrounds/Zones/`. |
| `class outdoorpvp_<zone> : public OutdoorPvPScript` | OPvP factory | Returns `OutdoorPvP*` for HP/NA/SI/TF/ZM. |
| `using <X>CommandScript : public CommandScript` | Chat command | `GetCommands()` returns nested `ChatCommandTable` for `.foo`, `.foo bar`, etc. |

The full list of WoLK 3.4.3 `*Script` base classes (the things scripts derive from) is enumerated in `scripting.md` §3.

---

## 4. Critical public methods / functions

Per-file. Below are recurring contract methods in the boss-AI pattern (the most common shape):

| Symbol | Purpose | Calls into |
|---|---|---|
| `BossAI::Reset()` | Wipe state, schedule pre-pull events, restore phase to default | `events.Reset`, `summons.DespawnAll` |
| `BossAI::JustEngagedWith(Unit* who)` | Encounter start: announce yell, schedule `events.ScheduleEvent`, set encounter to IN_PROGRESS | `instance->SetBossState(<id>, IN_PROGRESS)` |
| `BossAI::JustDied(Unit* killer)` | Encounter end: yell, despawn adds, set to DONE, fire achievement criteria | `instance->SetBossState(<id>, DONE)`, `DoCastAOE`, achievement triggers |
| `BossAI::EnterEvadeMode(EvadeReason)` | Wipe path: rewind all timers, despawn summons, set encounter to FAIL, reset doors | `summons.DespawnAll`, `instance->SetBossState(<id>, FAIL)` |
| `BossAI::UpdateAI(uint32 diff)` | Per-tick: drive `events.Update(diff)`, dispatch `events.ExecuteEvent()` switch, call `DoMeleeAttackIfReady` | `events.ExecuteEvent`, spell casts, movement |
| `InstanceScript::OnGameObjectCreate(GameObject*)` | Cache GUIDs of doors/event objects | `ObjectGuid` storage in instance |
| `InstanceScript::OnCreatureCreate(Creature*)` | Cache GUIDs of bosses/event NPCs | as above |
| `InstanceScript::SetBossState(uint32 id, EncounterState state)` | Drive door open/close, achievement checkpoints, save to DB | `Door::DoUseDoorOrButton`, `SaveToDB` |
| `InstanceScript::ReadSaveDataMore(istream&)` / `WriteSaveDataMore(ostream&)` | Per-instance persistence (custom flags beyond boss states) | `instance.data` row |
| `SpellScript::Register()` | Hook bag setup; called once per `SpellScript*` | `OnEffectHitTarget += SpellEffectFn(...)`, etc. |
| `AuraScript::Register()` | Same for auras | `OnEffectApply += AuraEffectApplyFn(...)`, etc. |
| `void AddSC_<file>()` | Registration: `new boss_lord_marrowgar();` `new RegisterSpellScript(spell_xxx);` etc. | Constructor side-effect → `ScriptRegistry<T>::AddScript` |

There is no single facade — each `*Script` ctor does its own `ScriptRegistry` insert.

---

## 5. Module dependencies

**Depends on:**
- `crates/wow-script/` (the `*Script` traits + registry) — see `scripting.md`.
- `crates/wow-ai/` (`BossAI`, `ScriptedAI`, `EventMap`, `SummonList`, `TaskScheduler`).
- `crates/wow-spell/` (for `SpellScript`/`AuraScript` and the spell engine).
- `crates/wow-instance/` and instance/dungeon-finder code (for `InstanceScript`).
- `crates/wow-pvp/`, `crates/wow-battleground/`, `crates/wow-outdoorpvp/` (for BG/OPvP/Battlefield).
- `crates/wow-pet/` (for Pet/* scripts).
- `crates/wow-conditions/` (for AchievementCriteriaScript & ConditionScript).
- `crates/wow-loot/`, `crates/wow-quest/`, `crates/wow-chat/` (for many NPC/quest/gossip/chat scripts).
- DB2 stores (achievement, criteria, spell, item, areatrigger, faction, …).

**Depended on by:**
- Nothing internal. Scripts are leaves; the framework calls into them, not vice versa. They are the **last** thing to migrate, after every dependency is stable.

---

## 6. SQL / DB queries (if any)

Individual scripts rarely emit raw queries. Two recurring exceptions:
- `InstanceScript::SaveToDB` / `ReadSaveDataMore` use `instance` table rows under the hood (handled by core, not script).
- A handful of holiday-event scripts and some larger instance scripts query `world_state`, `world` reference tables, or use `WorldDatabase.PQuery` for one-off lookups (e.g. `Naxxramas` checks `creature_template` for adds).

DBC/DB2 stores read indirectly (via `sObjectMgr`, `sSpellMgr`, `sAchievementMgr`):

| Store | What it loads | Read by |
|---|---|---|
| `SpellMgr` (Spell.db2 + spell_dbc, spell_proc, spell_target_position, …) | Spell metadata referenced by `SpellScript`/`AuraScript` | Every `Spells/*.cpp` |
| `AchievementMgr` (Achievement.db2, Criteria.db2, ModifierTree.db2) | Achievement criteria for `AchievementCriteriaScript` | `World/achievement_scripts.cpp`, instance scripts |
| `ObjectMgr` (creature_template, creature_template_addon, gameobject_template, areatrigger, areatrigger_scripts, conditions) | Per-entry script binding | Every script that resolves a `ScriptName` |
| `MapMgr` (Map.db2, MapDifficulty.db2) | Difficulty selection for raid/heroic versions | Most boss scripts (`IsHeroic()`, `IsTenMan()`, `Is25ManRaid()`) |

---

## 7. Wire-protocol packets (if any)

Scripts emit hundreds of packets. Highlights of the recurring wire-protocol surface:

| Opcode | Direction | Sent by |
|---|---|---|
| `SMSG_PLAY_SOUND` | server → client | Boss scripts, holiday event scripts (yells, mood SFX) |
| `SMSG_PLAY_OBJECT_SOUND` | server → client | Object-anchored sound (gunship horns, ICC throne hum) |
| `SMSG_TEXT_EMOTE` | server → client | Monster yells/emotes (`Talk(SAY_AGGRO)`) |
| `SMSG_CHAT` (variants) | server → client | Boss yells via `BroadcastText` |
| `SMSG_SPELL_GO` / `SMSG_SPELL_START` | server → client | Cast packets from `me->CastSpell` |
| `SMSG_AURA_UPDATE` (and friends) | server → client | Aura application via `AuraScript` |
| `SMSG_GAME_OBJECT_RESET_STATE` / `_CUSTOM_ANIM` | server → client | `GameObject::SetGoState`, animation triggers |
| `SMSG_AREA_TRIGGER_MESSAGE` | server → client | `at_*` scripts |
| `SMSG_GOSSIP_MESSAGE` / `SMSG_GOSSIP_POI` | server → client | Quest/gossip NPCs |
| `SMSG_QUEST_*` | server → client | Quest helper NPCs |
| `SMSG_RAID_INSTANCE_MESSAGE` / `SMSG_RAID_INSTANCE_INFO` | server → client | Instance bind, lockout |
| `SMSG_INSTANCE_ENCOUNTER_*` | server → client | Pull/wipe/end notifications |
| `SMSG_WEATHER` | server → client | Weather event scripts (love-is-in-air haze, hallow's-end smoke) |
| `CMSG_AREATRIGGER` | client → server | Triggers `AreaTriggerScript::OnTrigger` |
| `CMSG_GOSSIP_HELLO` / `CMSG_GOSSIP_SELECT_OPTION` | client → server | NPC gossip |
| `CMSG_QUEST_GIVER_ACCEPT_QUEST` | client → server | Triggers `ItemScript::OnQuestAccept`, quest scripts |

There is no opcode used **only** by scripts — they reuse the entire game protocol.

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-scripts` | `crate_dir` | 1 | 0 | `exists_empty` | crate exists; no active Rust source lines |
| `crates/wow-spell` | `crate_dir` | 1 | 0 | `exists_empty` | crate exists; no active Rust source lines |
| `crates/wow-pvp` | `crate_dir` | 1 | 0 | `exists_empty` | crate exists; no active Rust source lines |
| `crates/wow-battleground` | `crate_dir` | 0 | 0 | `missing_declared_path` | declared/proposed target does not exist |
| `crates/wow-pet` | `crate_dir` | 0 | 0 | `missing_declared_path` | declared/proposed target does not exist |
| `crates/wow-script` | `crate_dir` | 1 | 0 | `exists_empty` | crate exists; no active Rust source lines |
| `crates/wow-scripts/src/lib.rs` | `file` | 1 | 0 | `exists_empty` | file exists but has 0 lines |
| `crates/wow-scripts/Cargo.toml` | `file` | 1 | 11 | `exists_manifest` | manifest exists; not counted as active Rust source |
| `crates/wow-core` | `crate_dir` | 4 | 1153 | `exists_active` | crate exists |
| `crates/wow-constants` | `crate_dir` | 10 | 5477 | `exists_active` | crate exists |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-scripts/src/lib.rs` — **0 lines** (empty).
- `crates/wow-scripts/Cargo.toml` — depends on `wow-script`, `wow-core`, `wow-constants`. Nothing else.
- No subdirectory structure (no `northrend/`, no `events/`, no `spells/`, no `commands/`).

**What's implemented:** Nothing.

**What's missing vs C++:** The full content layer. Every boss, every instance, every quest helper, every spell script, every GM command, every holiday event, every OutdoorPvP zone, every pet AI, every Wintergrasp interaction. ~725 `.cpp` files / ~294k LOC.

**Suspicious / likely divergent (hypothesis pre-audit):**
- The Rust port will almost certainly **not** mirror the 1-file-per-encounter C++ structure verbatim. Expect a smaller "core encounters" set first (maybe ICC + Naxxramas + Ulduar bosses for WoLK relevance, plus Wintergrasp), then triage everything else.
- Many of the BC and old-world scripts (Karazhan, ZulAman, Sunwell, every Outland 5-man, every old-world dungeon) are functionally **dead content** for a WoLK 3.4.3 server's usage profile (max-level players ignore them entirely), so triage will deprioritize them in practice. They still need scripts to satisfy `creature_template.ScriptName` references during DB load — minimal stubs may suffice.
- `Spells/spell_*.cpp` files are individually large (1000+ lines each). They're the **most reused content** because every class-spell scripted behavior (e.g. shaman's "Fire Nova" exclusion of totems, hunter's "Kill Command" pet trigger, paladin's "Beacon of Light" healing redirection) lives there. **Migrating these is harder than migrating bosses** because they couple tightly with the spell engine in `wow-spell` (which is itself partial) — no shortcut.
- `World/npcs_special.cpp` is a grab-bag of ~3000+ lines covering innkeepers, generic banker, scryer/aldor faction switchers, generic guards, mailbox helpers, vendors, and dozens of one-off NPCs. Splitting it into Rust modules is unavoidable.

**Tests existing:** None.

---

## 9. Migration sub-tasks

Numbering: `#SCRIPTS.N`. Complexity: **L** (<1h), **M** (1–4h), **H** (4–12h), **XL** (>12h, split further). This list is intentionally long; expect to split each XL further at execution time.

### Phase A — scaffolding

- [ ] **#SCRIPTS.1** Create the `wow-scripts` directory layout: `northrend/`, `eastern_kingdoms/`, `kalimdor/`, `outland/`, `events/`, `spells/`, `commands/`, `world/`, `pet/`, `outdoor_pvp/`, `battlefield/`, `custom/`. One `mod.rs` per. (L)
- [ ] **#SCRIPTS.2** Add a `register_all` entry point that the world-server calls on init — the Rust analog of `AddScripts()`. With `inventory::submit!` it's mostly auto-triggered, but you still need the `mod` chain to compile in. (M)
- [ ] **#SCRIPTS.3** Define a stub helper module for the recurring "scripted boss" shape: a `BossAi` adapter wrapping `wow-ai::BossAI` that exposes `register_boss!(name, struct)`. (H)

### Phase B — Northrend (raids, then 5-mans, then zones)

#### Icecrown Citadel (`northrend/icecrown_citadel/`)

- [ ] **#SCRIPTS.10** `instance_icecrown_citadel.rs` — the `InstanceScript` (1420 lines C++). State machine, GUID cache, 12 boss states, achievement criteria. (XL — split into ICC.10a/10b/10c)
- [ ] **#SCRIPTS.11** `boss_lord_marrowgar.rs` (697 lines). Bone Spike Graveyard, Whirlwind, Coldflame, Bone Storm. (H)
- [ ] **#SCRIPTS.12** `boss_lady_deathwhisper.rs` (1081 lines). Adds, MC, mana shield. (XL)
- [ ] **#SCRIPTS.13** `boss_icecrown_gunship_battle.rs` (2243 lines). Vehicles, jet packs, dual-faction. (XL — split per faction + per role)
- [ ] **#SCRIPTS.14** `boss_deathbringer_saurfang.rs` (1257 lines). Mark of the Fallen Champion, Blood Beasts. (XL)
- [ ] **#SCRIPTS.15** `boss_festergut.rs` (457 lines). Inhale Blight, gas spore. (H)
- [ ] **#SCRIPTS.16** `boss_rotface.rs` (796 lines). Slime spray, ooze flood. (XL)
- [ ] **#SCRIPTS.17** `boss_professor_putricide.rs` (1488 lines). Three phases, abomination, choking gas. (XL)
- [ ] **#SCRIPTS.18** `boss_blood_prince_council.rs` (1341 lines). Trio fight. (XL)
- [ ] **#SCRIPTS.19** `boss_blood_queen_lana_thel.rs` (778 lines). Vampiric bite chain. (XL)
- [ ] **#SCRIPTS.20** `boss_valithria_dreamwalker.rs` (1284 lines). Healing-target encounter. (XL)
- [ ] **#SCRIPTS.21** `boss_sindragosa.rs` (1526 lines). Frost Beacons, Ice Tomb, Mystic Buffet. (XL)
- [ ] **#SCRIPTS.22** `boss_the_lich_king.rs` (2815 lines). 4 phases + Frostmourne Room. (XL — split 22a–22e per phase)
- [ ] **#SCRIPTS.23** `boss_sister_svalna.rs` (1487 lines). Crimson Hall path. (XL)
- [ ] **#SCRIPTS.24** `icecrown_citadel.rs` shared trash & helpers (1596 lines). Gauntlet, plagueworks, council servants, wing teleporters. (XL — split per wing)
- [ ] **#SCRIPTS.25** `go_icecrown_citadel_teleport.rs` (121 lines). (L)

#### Ulduar (`northrend/ulduar/`) — 12 boss files + instance + Halls of Stone + Halls of Lightning

- [ ] **#SCRIPTS.30** `instance_ulduar.rs`. (XL)
- [ ] **#SCRIPTS.31** Flame Leviathan vehicle encounter. (XL)
- [ ] **#SCRIPTS.32** Razorscale, Ignis, XT-002, Iron Council, Kologarn, Auriaya, Hodir, Thorim, Freya, Mimiron, General Vezax, Yogg-Saron, Algalon. (XL each → ~13 sub-tasks)
- [ ] **#SCRIPTS.33** Halls of Stone (3 bosses + tribunal event + instance script). (XL → ~5 sub-tasks)
- [ ] **#SCRIPTS.34** Halls of Lightning (4 bosses + instance script). (XL → ~5 sub-tasks)

#### Naxxramas (`northrend/naxxramas/`) — 16 boss files + instance

- [ ] **#SCRIPTS.40** `instance_naxxramas.rs`. (H)
- [ ] **#SCRIPTS.41** Arachnid quarter: Anub'rekhan, Faerlina, Maexxna. (H each → 3 sub-tasks)
- [ ] **#SCRIPTS.42** Plague quarter: Noth, Heigan, Loatheb. (H each → 3)
- [ ] **#SCRIPTS.43** Construct quarter: Patchwerk, Grobbulus, Gluth, Thaddius. (H each → 4; Thaddius is XL)
- [ ] **#SCRIPTS.44** Military quarter: Razuvious, Gothik, Four Horsemen. (H each → 3; Horsemen XL)
- [ ] **#SCRIPTS.45** Frostwyrm Lair: Sapphiron, Kel'Thuzad. (XL each → 2)
- [ ] **#SCRIPTS.46** `naxxramas.rs` shared. (M)

#### Crusaders' Coliseum (`northrend/crusaders_coliseum/`)

- [ ] **#SCRIPTS.50** Trial of the Champion (4 fights + instance). (XL → 5 sub-tasks)
- [ ] **#SCRIPTS.51** Trial of the Crusader: Northrend Beasts, Jaraxxus, Faction Champions, Twin Val'kyr, Anub'arak + instance. (XL → 6 sub-tasks)

#### Northrend 5-mans

- [ ] **#SCRIPTS.55** Azjol-Nerub (Krik'thir, Hadronox, Anub'arak + instance). (XL → 4)
- [ ] **#SCRIPTS.56** Ahn'kahet (5 bosses + instance + 1 helper). (XL → 7)
- [ ] **#SCRIPTS.57** Drak'Tharon Keep (Trollgore, Novos, King Dred, Tharon'ja + instance). (XL → 5)
- [ ] **#SCRIPTS.58** Frozen Halls (Forge of Souls, Pit of Saron, Halls of Reflection — 3 instance scripts × 3-4 bosses). (XL → ~12)
- [ ] **#SCRIPTS.59** Gundrak (5 bosses + instance). (XL → 6)
- [ ] **#SCRIPTS.60** Nexus 5-man + Eye of Eternity 25 (Malygos) + Oculus. (XL → ~10)
- [ ] **#SCRIPTS.61** Utgarde Keep + Utgarde Pinnacle. (XL → ~10)
- [ ] **#SCRIPTS.62** Violet Hold (random boss pool of 6 + instance). (XL → 8)
- [ ] **#SCRIPTS.63** Vault of Archavon (4 bosses + instance). (XL → 5)
- [ ] **#SCRIPTS.64** Chamber of Aspects (Onyxia 3.4 retuned, Obsidian Sanctum, Ruby Sanctum). (XL → 8+)
- [ ] **#SCRIPTS.65** Isle of Conquest BG. (XL)

#### Northrend zones (10 files)

- [ ] **#SCRIPTS.70** `zone_borean_tundra.rs`. (M-H per zone, ~depends on number of NPCs scripted)
- [ ] **#SCRIPTS.71** `zone_dalaran.rs`. (M)
- [ ] **#SCRIPTS.72** `zone_dragonblight.rs`. (M-H)
- [ ] **#SCRIPTS.73** `zone_grizzly_hills.rs`. (M)
- [ ] **#SCRIPTS.74** `zone_howling_fjord.rs`. (M-H)
- [ ] **#SCRIPTS.75** `zone_icecrown.rs`. (H)
- [ ] **#SCRIPTS.76** `zone_sholazar_basin.rs`. (M-H)
- [ ] **#SCRIPTS.77** `zone_storm_peaks.rs`. (M-H)
- [ ] **#SCRIPTS.78** `zone_wintergrasp.rs` (mostly references to `Battlefield/BattlefieldWG`). (M)
- [ ] **#SCRIPTS.79** `zone_zuldrak.rs`. (M-H)

### Phase C — Outland (BC content; lower priority for WoLK server)

- [ ] **#SCRIPTS.100** Black Temple — Illidan, Akama, the 8 mid-bosses. (XL → ~10)
- [ ] **#SCRIPTS.101** Sunwell Plateau (technically EK dir) — Kil'jaeden, M'uru, Brutallus, Felmyst, Eredar Twins, Kalecgos. (XL → ~10)
- [ ] **#SCRIPTS.102** Hyjal Summit — Anetheron, Kaz'rogal, Azgalor, Rage Winterchill, Archimonde. (XL → 7)
- [ ] **#SCRIPTS.103** Karazhan — Attumen, Moroes, Maiden, Opera, Curator, Aran, Netherspite, Nightbane, Prince. (XL → ~12)
- [ ] **#SCRIPTS.104** Tempest Keep — The Eye (Kael'thas), Mech, Bot, Arc. (XL → ~12)
- [ ] **#SCRIPTS.105** Coilfang — SP/SH/SV/UB. (XL → ~12)
- [ ] **#SCRIPTS.106** Hellfire Citadel — HFR/SH/BF/MT. (XL → ~12)
- [ ] **#SCRIPTS.107** Auchindoun — AC/SH/MT/SE. (XL → ~12)
- [ ] **#SCRIPTS.108** Gruul's Lair, Magtheridon's Lair. (XL → 4)
- [ ] **#SCRIPTS.109** Doomwalker, Doomlord Kazzak (world bosses). (M each)
- [ ] **#SCRIPTS.110** ZulAman, Zul'Gurub. (XL → ~12)
- [ ] **#SCRIPTS.111** Magisters' Terrace. (XL → 5)
- [ ] **#SCRIPTS.112** Outland zones (6 files). (M-H each → 6)
- [ ] **#SCRIPTS.113** OutdoorPvP HP/NA/SI/TF/ZM (5 zones). (H each → 5)

### Phase D — old world (very low priority)

- [ ] **#SCRIPTS.130** Eastern Kingdoms 16 zone files. (M each → 16)
- [ ] **#SCRIPTS.131** EK dungeons: Deadmines, Gnomeregan, Stockade, ShadowfangKeep, Stratholme, Scholomance, Karazhan (counted above), Scarlet Monastery, Scarlet Enclave, Sunken Temple, Uldaman, Blackrock Mountain wings (UBRS/LBRS/MC/BWL/BRD). (XL each)
- [ ] **#SCRIPTS.140** Kalimdor 18 zone files. (M each → 18)
- [ ] **#SCRIPTS.141** Kalimdor dungeons: Onyxia, AQ20/AQ40, Dire Maul, Maraudon, Wailing Caverns, Razorfen Downs/Kraul, Blackfathom Deeps, Ragefire Chasm, Zul'Farrak, all 7 Caverns of Time instances. (XL each)

### Phase E — `Spells/` (one giant per-class file each; biggest engineering surface in this whole list)

- [ ] **#SCRIPTS.200** `spell_dk.rs`. (XL — every DK spell with custom logic: Death Coil, Death Grip, Strangulate, Anti-Magic Shell, Bone Shield, Death and Decay, Frost Strike, Howling Blast, Icy Touch, Mind Freeze, Obliterate, Plague Strike, Rune Strike, Scourge Strike, Unbreakable Armor, etc.)
- [ ] **#SCRIPTS.201** `spell_druid.rs`. (XL)
- [ ] **#SCRIPTS.202** `spell_hunter.rs`. (XL)
- [ ] **#SCRIPTS.203** `spell_mage.rs`. (XL)
- [ ] **#SCRIPTS.204** `spell_paladin.rs`. (XL)
- [ ] **#SCRIPTS.205** `spell_priest.rs`. (XL)
- [ ] **#SCRIPTS.206** `spell_rogue.rs`. (XL)
- [ ] **#SCRIPTS.207** `spell_shaman.rs`. (XL)
- [ ] **#SCRIPTS.208** `spell_warlock.rs`. (XL)
- [ ] **#SCRIPTS.209** `spell_warrior.rs`. (XL)
- [ ] **#SCRIPTS.210** `spell_pet.rs`. (XL)
- [ ] **#SCRIPTS.211** `spell_generic.rs`. (XL — non-class one-off spells; the largest of this set)
- [ ] **#SCRIPTS.212** `spell_item.rs`. (XL — trinkets, consumables)
- [ ] **#SCRIPTS.213** `spell_quest.rs`. (XL — quest reward and quest-step spells)

### Phase F — `Commands/` (GM chat commands)

One sub-task per file (42 total). Each is L–M.

- [ ] **#SCRIPTS.300** `cs_account.rs`. (M)
- [ ] **#SCRIPTS.301** `cs_achievement.rs`. (M)
- [ ] **#SCRIPTS.302** `cs_ahbot.rs`. (M)
- [ ] **#SCRIPTS.303** `cs_arena.rs`. (M)
- [ ] **#SCRIPTS.304** `cs_ban.rs`. (M)
- [ ] **#SCRIPTS.305** `cs_battlenet_account.rs`. (M)
- [ ] **#SCRIPTS.306** `cs_bf.rs` (battlefield). (L)
- [ ] **#SCRIPTS.307** `cs_cast.rs`. (M)
- [ ] **#SCRIPTS.308** `cs_character.rs`. (M)
- [ ] **#SCRIPTS.309** `cs_cheat.rs`. (M)
- [ ] **#SCRIPTS.310** `cs_debug.rs`. (M)
- [ ] **#SCRIPTS.311** `cs_deserter.rs`. (L)
- [ ] **#SCRIPTS.312** `cs_disable.rs`. (L)
- [ ] **#SCRIPTS.313** `cs_event.rs`. (L)
- [ ] **#SCRIPTS.314** `cs_gm.rs`. (M)
- [ ] **#SCRIPTS.315** `cs_go.rs`. (M)
- [ ] **#SCRIPTS.316** `cs_gobject.rs`. (M)
- [ ] **#SCRIPTS.317** `cs_group.rs`. (L)
- [ ] **#SCRIPTS.318** `cs_guild.rs`. (M)
- [ ] **#SCRIPTS.319** `cs_honor.rs`. (L)
- [ ] **#SCRIPTS.320** `cs_instance.rs`. (M)
- [ ] **#SCRIPTS.321** `cs_learn.rs`. (M)
- [ ] **#SCRIPTS.322** `cs_lfg.rs`. (M)
- [ ] **#SCRIPTS.323** `cs_list.rs`. (M)
- [ ] **#SCRIPTS.324** `cs_lookup.rs`. (M)
- [ ] **#SCRIPTS.325** `cs_message.rs`. (L)
- [ ] **#SCRIPTS.326** `cs_misc.rs`. (M)
- [ ] **#SCRIPTS.327** `cs_mmaps.rs`. (M)
- [ ] **#SCRIPTS.328** `cs_modify.rs`. (M)
- [ ] **#SCRIPTS.329** `cs_npc.rs`. (M)
- [ ] **#SCRIPTS.330** `cs_pet.rs`. (L)
- [ ] **#SCRIPTS.331** `cs_quest.rs`. (M)
- [ ] **#SCRIPTS.332** `cs_rbac.rs`. (M)
- [ ] **#SCRIPTS.333** `cs_reload.rs`. (M)
- [ ] **#SCRIPTS.334** `cs_reset.rs`. (M)
- [ ] **#SCRIPTS.335** `cs_scene.rs`. (L)
- [ ] **#SCRIPTS.336** `cs_send.rs`. (L)
- [ ] **#SCRIPTS.337** `cs_server.rs`. (M)
- [ ] **#SCRIPTS.338** `cs_tele.rs`. (L)
- [ ] **#SCRIPTS.339** `cs_ticket.rs`. (M)
- [ ] **#SCRIPTS.340** `cs_titles.rs`. (L)
- [ ] **#SCRIPTS.341** `cs_wp.rs` (waypoints). (M)

### Phase G — `World/` (cross-zone shared scripts)

- [ ] **#SCRIPTS.400** `npcs_special.rs` (3000+ lines C++). **Split per-NPC group**. (XL)
- [ ] **#SCRIPTS.401** `npc_professions.rs`. (H)
- [ ] **#SCRIPTS.402** `npc_guard.rs`. (M)
- [ ] **#SCRIPTS.403** `boss_emerald_dragons.rs` (Ysondre, Lethon, Emeriss, Taerar — world bosses). (XL → 5)
- [ ] **#SCRIPTS.404** `go_scripts.rs` (generic gameobjects). (H)
- [ ] **#SCRIPTS.405** `item_scripts.rs`. (M)
- [ ] **#SCRIPTS.406** `areatrigger_scripts.rs`. (H)
- [ ] **#SCRIPTS.407** `achievement_scripts.rs`. (H)
- [ ] **#SCRIPTS.408** `chat_log.rs`, `action_ip_logger.rs`, `boosted_xp.rs`, `duel_reset.rs`. (L each → 4)
- [ ] **#SCRIPTS.409** `conversation_scripts.rs`, `scene_scripts.rs`. (L each)

### Phase H — `Pet/`, `Battlefield/`, `Custom/`

- [ ] **#SCRIPTS.500** `pet_dk.rs`, `pet_hunter.rs`, `pet_mage.rs`, `pet_priest.rs`, `pet_shaman.rs`, `pet_generic.rs`. (M each → 6)
- [ ] **#SCRIPTS.501** `BattlefieldWG.rs` — Wintergrasp full implementation. (XL — coupled to `wow-battlefield`)
- [ ] **#SCRIPTS.502** `Custom/` — empty placeholder. (—)

### Phase I — `Events/` (holiday content scripts)

Covered in `events.md`. Cross-reference: each event file in `scripts/Events/` is **content** that depends on the **scheduler** in `src/server/game/Events/GameEventMgr` which `events.md` documents.

- [ ] **#SCRIPTS.600** `events/brewfest.rs`, `childrens_week.rs`, `darkmoon_faire.rs`, `fireworks_show.rs`, `hallows_end.rs`, `love_is_in_the_air.rs`, `lunar_festival.rs`, `midsummer.rs`, `operation_gnomeregan.rs`, `pilgrims_bounty.rs`, `winter_veil.rs`, `zalazane_fall.rs`. (H each → 12)

---

## 10. Regression tests to write

These are encounter-level acceptance tests; one per major sub-system. **All depend on `#SCRIPTING.*` framework being live first**.

- [ ] Test: an `inventory::submit!` registered `boss_lord_marrowgar` is reachable via `MapManager` for a creature whose template `ScriptName="boss_lord_marrowgar"` and produces a `Box<dyn CreatureAI>` on demand.
- [ ] Test: `instance_icecrown_citadel` `set_boss_state(MARROWGAR, DONE)` opens the door GUID it cached during `on_creature_create`.
- [ ] Test: ICC instance state survives a server restart (re-load via `instance` row).
- [ ] Test: a `spell_dk` aura (Bone Shield) intercepts melee damage as expected.
- [ ] Test: GM command `.gm fly on` toggles `MOVEMENTFLAG_CAN_FLY` and emits the matching opcode to the issuing player.
- [ ] Test: `at_<some_trigger>` fires once on entry and zero times on exit (with `entered=true`).
- [ ] Test: Wintergrasp battle starts on schedule, bestows the Vault of Archavon access aura, and tears it down on end.
- [ ] Test: a Northrend zone NPC (e.g. quest helper) responds to gossip with the right options for a player with the matching quest state.
- [ ] Test: holiday event `winter_veil` swaps the Greatfather Winter NPC vendor when active and reverts when inactive (couples with `events.md`).
- [ ] Test: `boss_emerald_dragons.rs` Ysondre spawn rotation respects the world-event window (couples with `events.md`).

---

## 11. Notes / gotchas

- **This is the longest tail in the project.** Don't try to enumerate sub-tasks at full granularity up-front; the list above is shaped to be a **triage menu** rather than a roadmap. Pick the WoLK-relevant content (Northrend raids + 5-mans + zones, Wintergrasp, Spells/spell_*) and treat the rest as "needed only for DB integrity" (creature_template script names must resolve to *something* even if it's a no-op stub).
- **Dependency order is brutal.** A boss script can't compile until its `BossAI` parent (`wow-ai`), its summon helpers (`wow-ai::SummonList`), its event scheduler (`wow-ai::EventMap`), its instance script base (`wow-instance::InstanceScript`), and most of `wow-spell` are usable. Do **not** start scripts until those are stable enough not to keep churning underneath you.
- **`Spells/spell_*.cpp` is the single biggest blocker for actual gameplay.** If `spell_dk.cpp` / `spell_warrior.cpp` etc. are unmigrated, every DK and warrior is non-functional at endgame. Prioritize Phase E in parallel with Phase B raids — not after.
- C++ instance scripts use `std::ostringstream`/`std::istringstream` for `WriteSaveDataMore`/`ReadSaveDataMore`. The Rust port should use a small `serde` envelope (or fixed-format text mirror) — pick one early because all instance scripts inherit from it.
- Many boss scripts use `Talk(SAY_X)` which under the hood reads from `creature_text` table. The text table is data; the IDs (`SAY_AGGRO`, `SAY_DEATH`, `EMOTE_X`) are constants per-encounter — preserve the constant names for grep-ability.
- C++ frequently pattern: `if (instance && instance->GetData(DATA_X) == DONE) { … }`. The Rust shape will most cleanly use an `enum BossId` indexing into a `[EncounterState; N]` array on the instance script — see how `wow-instance` (when it lands) settles this.
- WoLK-specific: many old-world (Vanilla/BC) instance scripts in this tree also reference `LFG`/`Random Dungeon Finder` mechanics that are post-WoLK. Carefully audit which scripts assume LFG behavior (the `cs_lfg.cpp` admin command and a few instance scripts).
- **Don't port everything.** A minimal viable WoLK 3.4.3 server can ship with: ICC + Naxx + Ulduar + ToC bosses, all WoLK 5-mans, Vault of Archavon, Onyxia 3.4, Wintergrasp, a few hundred high-traffic spell scripts (DK/Pally + every PvP-relevant proc), and ~50 GM commands. Everything else can stub-load.
- The C++ `ScriptLoader.cpp.in.cmake` template auto-generates the master `AddScripts()` function. Rust avoids this by having `inventory::submit!` register at link time inside each module — but you must explicitly `mod` every submodule into the crate root, or the linker will dead-strip the registrations. **This is the same trap as the packet handlers**: forgetting `mod foo;` makes the script silently invisible.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class boss_xxx : public CreatureScript { class boss_xxx_AI : public BossAI { … } }` | `pub struct BossXxxAi { … }` impl `BossAi` trait + `register_creature_ai!(BossXxxAi)` | One file per encounter; flat module structure — drop the nested `_AI` class. |
| `class instance_xxx : public InstanceMapScript` | `pub struct InstanceXxx { state: InstanceState, … }` impl `InstanceScript` trait | One file per dungeon. |
| `class spell_xxx : public SpellScriptLoader` | `pub fn spell_xxx() -> SpellScriptDescriptor { … }` plus `register_spell_script!` | Coupled to `wow-spell`. |
| `class npc_xxx : public CreatureScript` | `pub struct NpcXxxAi { … }` + `register_creature_ai!` | NPC scripts are ordinary creature scripts; the `npc_` prefix is style. |
| `class go_xxx : public GameObjectScript` | `pub struct GoXxx { … }` + `register_game_object_ai!` | — |
| `class at_xxx : public AreaTriggerScript` | `pub struct AtXxx;` + `register_area_trigger!` | — |
| `class xxx_CommandScript : public CommandScript { GetCommands() { return … } }` | a `pub fn xxx_commands() -> CommandTable` + `register_commands!` macro | The chat command builder API is a cross-cutting decision (also see `chat.md` and `scripting.md` #SCRIPTING.17). |
| `void AddSC_xxx()` | (none — `inventory::submit!` replaces this) | Aggregator function disappears. |
| `<expansion>_script_loader.cpp` (e.g. `northrend_script_loader.cpp`) | `crates/wow-scripts/src/northrend/mod.rs` listing `pub mod icecrown_citadel; pub mod ulduar; …` | Same intent (compile-link aggregation) but driven by `mod` declarations. |
| `Talk(SAY_X)` / `creature_text` table | `talk!(self, "SAY_X")` macro that resolves to a `creature_text` row at runtime | Preserve text-id constants. |
| `events.ScheduleEvent(EVENT_X, 5s)` | `self.events.schedule(EventId::X, Duration::from_secs(5))` | `wow-ai::EventMap` design is upstream. |
| `me->CastSpell(target, SPELL_X)` | `unit.cast_spell(target, SpellId::X)` | Through `wow-spell`. |

---

*Template version: 1.0 (2026-05-01).*

---

## 13. Audit (2026-05-01)

**Verdict: ❌ confirmed — `crates/wow-scripts/src/lib.rs` is empty (0 lines).**

```
$ wc -l crates/wow-scripts/src/lib.rs
0 crates/wow-scripts/src/lib.rs
```

`crates/wow-scripts/Cargo.toml` deps: `wow-script`, `wow-core`, `wow-constants` — the dependency edges are correct (content depends on framework) but neither side has any code. There is no `northrend/`, no `easternkingdoms/`, no `kalimdor/`, no `outland/`, no `commands/`, no `spells/`, no `events/`, no `pet/`, no `world/` submodule directory. **Zero** boss scripts, **zero** instance scripts, **zero** GM commands.

C++ comparison (from §2):
- ~725 `.cpp` files in `src/server/scripts/`
- ~294,137 lines aggregate
- ICC alone: 16 files, ~20,387 lines (largest single dungeon)
- Northrend (the WoLK content): 169 files
- Spells: 15 files holding hundreds of `SpellScript`/`AuraScript` per file (estimated ~3,000 spell scripts total across `Spells/`)

**No silent-default bug** — without `wow-script` framework (also ❌, see scripting.md audit), there is no hookpoint to silently no-op against. Bosses simply spawn as inert mobs running default `CreatureAI` (whatever lands in `wow-ai`); GMs have no `.gm fly` etc.; holidays do nothing; OutdoorPvP zones offer no objectives.

**Hard blocker:** `wow-scripts` is double-blocked. (1) #SCRIPTING.* must land in `wow-script` first to provide the trait set. (2) Every script also needs the underlying gameplay primitives (spell engine, instance state machine, AI base classes, conditions, gossip flow, vehicle hooks, conversation/scene). Realistically only `Commands/` (42 files) and a handful of `World/` helper NPCs (innkeepers, banker scripts) are tractable in the first year of porting; the boss/instance/spell content is a multi-year backlog.

**Recommendation:** Treat this doc as the canonical content roadmap, not as work that ships in the next migration cycle. Prioritise framework (scripting.md) + a single dungeon vertical slice (e.g. one ICC boss with its instance script) as the first end-to-end demonstration; defer the bulk to post-1.0.
