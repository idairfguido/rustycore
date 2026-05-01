#!/bin/bash
# Migration script for character.rs - Replace legacy creature system with MapManager global

cd /home/server/rustycore_ARCHIVED_20260312/crates/wow-world/src/handlers

# Backup
cp character.rs character.rs.backup.$(date +%s)

# Replace 1: Creature spawn in initial spawn section (around line 1924)
# This is a complex replacement that needs manual handling

cat << 'EOF'
════════════════════════════════════════════════════════════════════════════════
MANUAL MIGRATION STEPS FOR character.rs
════════════════════════════════════════════════════════════════════════════════

1. AROUND LINE 1924 (Initial creature spawn):
   REPLACE:
       self.creatures.insert(guid, ai);
   
   WITH:
       let creature = WorldCreature {
           guid,
           entry: ai.entry,
           level: ai.level,
           is_alive: true,
           current_hp: ai.max_hp,
           max_hp: ai.max_hp,
           position: ai.position,
           home_pos: ai.position,
           state: CreatureState::Idle,
           move_target: None,
           corpse_despawn_at: None,
           npc_flags: ai.npc_flags,
           unit_flags: ai.unit_flags,
           aggro_radius: ai.aggro_radius,
           min_dmg: ai.min_dmg,
           max_dmg: ai.max_dmg,
           create_data, // Make sure create_data is available in scope
       };
       self.spawn_creature_global(creature);

2. AROUND LINE 1937 (visible_creatures update):
   REPLACE:
       self.visible_creatures = self.creatures.keys().cloned().collect();
       let count_mobs = self.creatures.values().filter(|a| a.npc_flags == 0).count();
       let count_npcs = self.creatures.values().filter(|a| a.npc_flags > 0).count();
   
   WITH:
       let visible = self.get_visible_creatures_from_map();
       let count_mobs = visible.iter().filter(|c| c.npc_flags == 0).count();
       let count_npcs = visible.iter().filter(|c| c.npc_flags > 0).count();

3. AROUND LINE 2082 (dynamic spawn):
   REPLACE:
       self.creatures.insert(guid, ai);
   
   WITH (same as #1):
       let creature = WorldCreature { ... };
       self.spawn_creature_global(creature);

4. AROUND LINE 2102 (despawn):
   REPLACE:
       self.creatures.remove(g);
   
   WITH:
       self.despawn_creature_global(*g);

5. AROUND LINE 2106 and 2201 (visible checks):
   REPLACE:
       if !self.visible_creatures.contains(&guid)
   
   WITH:
       if !self.is_creature_visible(&guid)

6. AROUND LINE 2713-2714 (Gossip hello):
   REPLACE:
       let npc_flags = self.creatures.get(&hello.unit).map(|c| c.npc_flags).unwrap_or(0);
       let entry = self.creatures.get(&hello.unit).map(|c| c.entry).unwrap_or(0);
   
   WITH:
       let npc_flags = self.get_creature_npc_flags(&hello.unit);
       let entry = self.get_creature_entry(&hello.unit);

7. AROUND LINE 2906 (Trainer list):
   REPLACE:
       let npc_flags = self.creatures.get(&hello.unit).map(|c| c.npc_flags).unwrap_or(0);
   
   WITH:
       let npc_flags = self.get_creature_npc_flags(&hello.unit);

8. AROUND LINE 3085 (Vendor entry):
   REPLACE:
       let entry = match self.creatures.get(&vendor_guid) {
           Some(c) => c.entry,
           None => { ... }
       };
   
   WITH:
       let entry = self.get_creature_entry(&vendor_guid);
       if entry == 0 {
           warn!("Vendor {:?} not found in global map", vendor_guid);
           return;
       }

9. AROUND LINE 3227 (Buy item):
   REPLACE:
       let vendor_entry = match self.creatures.get(&buy.vendor_guid) {
           Some(c) => c.entry,
           None => return,
       };
   
   WITH:
       let vendor_entry = self.get_creature_entry(&buy.vendor_guid);
       if vendor_entry == 0 {
           return;
       }

10. AROUND LINE 4450 (Logout cleanup):
    REPLACE:
        self.creatures.clear();
    
    WITH:
        // Creatures are now global - no per-session cleanup needed
        // Optionally: remove player from all grids
        if let Some(pos) = self.player_position {
            if let Some(ref map_manager) = self.map_manager {
                let mut guard = map_manager.write();
                if let Some(inst) = guard.get_instance(self.current_map_id as i32, 0) {
                    inst.remove_player(self.player_guid.unwrap_or(ObjectGuid::EMPTY), pos.x, pos.y);
                }
            }
        }

════════════════════════════════════════════════════════════════════════════════
IMPORTS TO ADD AT TOP OF character.rs:
════════════════════════════════════════════════════════════════════════════════

Add to existing imports:
use crate::map_manager::WorldCreature;
use crate::map_manager::CreatureState;

════════════════════════════════════════════════════════════════════════════════
EOF
