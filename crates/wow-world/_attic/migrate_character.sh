#!/bin/bash
# Script de migración para character.rs

cd /home/server/rustycore_ARCHIVED_20260312/crates/wow-world/src/handlers

# Backup
cp character.rs character.rs.backup.$(date +%s)

# 1. Agregar import del trait de integración al inicio del archivo (después de los imports existentes)
sed -i '/use crate::session::/a use crate::session_map_integration::SessionMapIntegration;' character.rs

# 2. Reemplazar self.creatures.insert(guid, ai); en línea 1924
sed -i 's/self\.creatures\.insert(guid, ai);/\/\/ Criatura se registra en MapManager global al hacer spawn\n            \/\/ El AI se maneja por separado en el sistema global/' character.rs

# 3. Reemplazar snapshot visible set (línea 1937)
sed -i 's/self\.visible_creatures = self\.creatures\.keys()\.cloned()\.collect();/\/\/ Visible creatures se obtienen del MapManager global/' character.rs

# 4. Reemplazar conteos de criaturas (líneas 1943-1944)
sed -i 's/self\.creatures\.values()\.filter(|a| a\.npc_flags == 0)\.count()/self.get_visible_creatures().iter().filter(|c| c.npc_flags == 0).count()/' character.rs
sed -i 's/self\.creatures\.values()\.filter(|a| a\.npc_flags > 0)\.count()/.get_visible_creatures().iter().filter(|c| c.npc_flags > 0).count()/' character.rs

# 5. Reemplazar insert en dynamic spawn (línea 2082)
sed -i 's/self\.creatures\.insert(guid, ai);/\/\/ Criatura registrada en MapManager global/' character.rs

# 6. Reemplazar remove de criaturas (línea 2102)
sed -i 's/self\.creatures\.remove(g);/self.despawn_creature_global(*g);/' character.rs

# 7. Reemplazar visible_creatures assignments
sed -i 's/self\.visible_creatures = new_visible_creatures;/\/\/ Actualización de visibilidad manejada por MapManager/' character.rs

# 8. Reemplazar self.creatures.get para gossip (líneas 2713-2714)
sed -i 's/let npc_flags = self\.creatures\.get(&hello\.unit)\.map(|c| c\.npc_flags)\.unwrap_or(0);/let npc_flags = self.get_creature_npc_flags(\&hello.unit);/' character.rs
sed -i 's/let entry = self\.creatures\.get(&hello\.unit)\.map(|c| c\.entry)\.unwrap_or(0);/let entry = self.get_creature_entry(\&hello.unit);/' character.rs

# 9. Reemplazar vendor check (línea 2906)
sed -i 's/let npc_flags = self\.creatures\.get(&hello\.unit)\.map(|c| c\.npc_flags)\.unwrap_or(0);/let npc_flags = self.get_creature_npc_flags(\&hello.unit);/' character.rs

# 10. Reemplazar vendor entry (línea 3085)
sed -i 's/let entry = match self\.creatures\.get(&vendor_guid) {/let entry = self.get_creature_entry(\&buy.vendor_guid);\n        if entry == 0 {/' character.rs

# 11. Reemplazar clear de criaturas (línea 4450)
sed -i 's/self\.creatures\.clear();/\/\/ Las criaturas se manejan globalmente, no por sesión/' character.rs

echo "Migración completada. Verificando..."
